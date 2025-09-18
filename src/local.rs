use std::{
    collections::VecDeque,
    io,
    path::{Path, PathBuf},
};

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fileutil;

/// Errors that can occur while parsing the manifest file.
#[derive(Debug, Error)]
pub enum ModManifestError {
    /// The manifest file does not have any entries.
    #[error("no entries found in the manifest file")]
    Invalid,
    /// ZIP file does not contains the manifest file.
    #[error(
        "the manifest file could not be found. It may be misspelled or have the extension `.yml`."
    )]
    NotFound,
    /// Failed to parse the manifest file.
    #[error(transparent)]
    Parse(#[from] serde_yaml_ng::Error),
    /// Failed to parse the ZIP file. Broken ZIP format.
    #[error(transparent)]
    Zip(#[from] zip_search::ZipSearchError),
}

/// Represents the `everest.yaml` manifest file that defines a mod.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct ModManifest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "DLL")]
    dll: Option<String>,
    #[serde(rename = "Dependencies")]
    pub dependencies: Option<Vec<Dependency>>,
    #[serde(rename = "OptionalDependencies")]
    pub optional_dependencies: Option<Vec<Dependency>>,
}

/// Dependency specification for required or optional mod dependencies.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct Dependency {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: Option<String>,
}

impl ModManifest {
    /// Deserialize an instance of type ModManifest from bytes of YAML text.
    ///
    /// # Errors
    /// - `Parse`: Failed to parse YAML format.
    /// - `Invalid`: The manifest file does not have any entries.
    pub fn from_slice(yaml_bytes: &[u8]) -> Result<Self, ModManifestError> {
        // NOTE: We always need first entry from this collection since that is the primal mod, so we use the `VecDeque<T>` here instead of the `Vec<T>`.
        let mut manifest_entries: VecDeque<Self> = serde_yaml_ng::from_slice(yaml_bytes)?;

        // Attempt to retrieve the first entry without unnecessary cloning or element shifting.
        match manifest_entries.pop_front() {
            Some(entry) => Ok(entry),
            None => Err(ModManifestError::Invalid),
        }
    }
}

/// Information about a locally installed mod.
#[derive(Debug, Clone)]
pub struct LocalMod {
    /// Path to the local mod file which contains the mod's assets and manifest
    pub location: PathBuf,
    /// Mod manifest resides in the mod file
    pub manifest: ModManifest,
    /// Computed XXH64 hash of the file for update check
    checksum: OnceCell<String>,
}

impl LocalMod {
    /// Returns a value of this type from the given file path by extracting and parsing the manifest.
    ///
    /// # Errors
    /// - `NotFound`: The manifest file not found in given path.
    /// - `Invalid`: The manifest file does not have any entries.
    /// - `Parse`: Failed to parse YAML format.
    /// - `Zip`: Broken ZIP format.
    ///
    /// # Example
    /// ```no_run
    /// use std::path::Path;
    ///
    /// use mod_manager::local::{create_local_mod_from_path, ModManifestError};
    ///
    /// let mod_path = Path::new("./test/test-mod.zip");
    /// match LocalMod::from_path(mod_path) {
    ///     Ok(local_mod) => {
    ///         println!("Loaded mod: {} version {}", local_mod.manifest.name, local_mod.manifest.version);
    ///     }
    ///     Err(e) => {
    ///         println!("An error occurred: {}", e);
    ///     }
    /// }
    /// ```
    pub fn from_path<P: AsRef<Path>>(mod_path: &P) -> Result<Self, ModManifestError> {
        let manifest_bytes = zip::find_manifest_in_zip(mod_path)?;
        let manifest = ModManifest::from_slice(&manifest_bytes)?;
        Ok(Self {
            location: mod_path.as_ref().to_path_buf(),
            manifest,
            checksum: OnceCell::new(),
        })
    }

    /// Compute checksum if not already computed, then cache it.
    pub fn checksum(&self) -> io::Result<&str> {
        self.checksum
            .get_or_try_init(|| fileutil::hash_file(&self.location))
            .map(|hash| hash.as_str())
    }

    /// Loads all local mods from the provided archive paths.
    ///
    /// # Notes
    /// Sometimes, `everest.yaml` file may not be present in the mod archive.
    /// In such cases, the function will log a warning and skip that archive.
    ///
    /// # Errors
    /// This function does not return errors directly. Instead, it logs errors when the manifest file could not be parsed or invalid.
    ///
    /// It's because we cannot do anything if some of the mod archives are broken but that's are not critical to stop the whole process.
    ///
    /// # Example
    /// ```no_run
    /// use std::path::PathBuf;
    /// use mod_manager::local::LocalMod;
    ///
    /// let archive_paths = vec![Path::new("./test/test-mod.zip")];
    /// let local_mods = LocalMod::load_local_mods(&archive_paths);
    /// for local_mod in local_mods {
    ///     println!("Loaded mod: {} version {}", local_mod.manifest.name, local_mod.manifest.version);
    /// }
    /// ```
    pub fn load_local_mods<P: AsRef<Path> + Sync>(archive_paths: &[P]) -> Vec<LocalMod> {
        use rayon::prelude::*;

        tracing::info!("Found {} mod archives to load", archive_paths.len());
        tracing::info!("Start parsing archive files.");
        let local_mods: Vec<LocalMod> = archive_paths
            .par_iter()
            .filter_map(|archive_path| match LocalMod::from_path(archive_path) {
                Ok(local_mod) => Some(local_mod),
                Err(e) if matches!(e, ModManifestError::NotFound) => {
                    tracing::warn!("{:?}: {}", archive_path.as_ref().file_name(), e);
                    None
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to load mod from {}: {}",
                        archive_path.as_ref().display(),
                        e
                    );
                    None
                }
            })
            .collect();
        tracing::info!("Successfully loaded {} local mods", local_mods.len());

        local_mods
    }
}

mod zip {
    //! Functions related to ZIP file operations.
    use std::path::Path;

    use zip_search::ZipSearcher;

    use super::ModManifestError;

    /// Finds manifest file in the ZIP file and returns the bytes of its contents.
    ///
    /// # Errors
    /// - `NotFound`: The manifest file not found in given path.
    /// - `Zip`: Broken ZIP format.
    ///
    /// # Example
    /// ```no_run
    /// use std::path::Path;
    /// use mod_manager::local::{find_manifest_in_zip, ModManifestError};
    ///
    /// let mod_path = Path::new("path/to/mod.zip");
    /// match find_manifest_in_zip(mod_path) {
    ///     Ok(manifest_bytes) => {
    ///         // Process the manifest bytes
    ///         println!("Manifest found with {} bytes", manifest_bytes.len());
    ///         // let manifest = ModManifest::from_yaml(&manifest_bytes).unwrap();
    ///     }
    ///     Err(ModManifestError::NotFound) => {
    ///         println!("Manifest file not found in the ZIP archive.");
    ///     }
    ///     Err(e) => {
    ///         println!("An error occurred: {}", e);
    ///     }
    /// }
    /// ```
    pub(super) fn find_manifest_in_zip<P: AsRef<Path>>(
        file_path: &P,
    ) -> Result<Vec<u8>, ModManifestError> {
        const MANIFEST_FILE_NAME: &str = "everest.yaml";

        let mut zip_searcher = ZipSearcher::new(file_path)?;
        match zip_searcher.find_file(MANIFEST_FILE_NAME) {
            Ok(Some(entry)) => {
                let mut buffer = zip_searcher.read_file(&entry)?;
                // Check for UTF-8 BOM and remove if present
                if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    buffer.drain(0..3);
                }
                Ok(buffer)
            }
            Ok(None) => Err(ModManifestError::NotFound),
            Err(err) => Err(ModManifestError::Zip(err)),
        }
    }

    #[cfg(test)]
    mod tests_for_zip {
        use super::*;
        use std::path::PathBuf;
        #[test]
        fn test_find_manifest_in_zip_valid() {
            let mod_path = PathBuf::from("./test/test-mod.zip");
            let result = find_manifest_in_zip(&mod_path);
            assert!(result.is_ok());
            let manifest_bytes = result.unwrap();
            assert!(!manifest_bytes.is_empty());
        }

        #[test]
        fn test_find_manifest_in_zip_invalid() {
            let mod_path = PathBuf::from("./test/missing-manifest.zip");
            let result = find_manifest_in_zip(&mod_path);
            assert!(result.is_err());
            assert!(
                result
                    .err()
                    .is_some_and(|e| matches!(e, ModManifestError::NotFound))
            );
        }
    }
}

#[cfg(test)]
mod tests_for_files {

    use super::*;

    #[test]
    fn test_from_yaml_parse_valid_manifest() {
        let yaml = r#"
        - Name: TestMod
          Version: 1.0.0
        "#;

        let result = ModManifest::from_slice(yaml.as_bytes());
        assert!(result.is_ok());
        let manifest = result.unwrap();

        assert_eq!(manifest.name, "TestMod");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn test_from_yaml_parse_invalid_manifest() {
        let yaml = r#"
        TestMod
          Version: 1.0.0
        "#;

        let result = ModManifest::from_slice(yaml.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_from_yaml_parse_empty_manifest() {
        let yaml = b"[]";

        let result = ModManifest::from_slice(yaml);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .is_some_and(|e| matches!(e, ModManifestError::Invalid))
        );
    }
}

#[cfg(test)]
mod tests_local_mod {
    use super::*;

    #[test]
    fn test_checksum_computation() {
        let mod_path = PathBuf::from("./test/test-mod.zip");
        let local_mod = LocalMod::from_path(&mod_path).unwrap();
        let checksum = local_mod.checksum().unwrap();
        assert!(!checksum.is_empty());
    }

    #[test]
    fn test_from_path_valid_file() {
        let valid_path = PathBuf::from("./test/test-mod.zip");
        let result = LocalMod::from_path(&valid_path);
        assert!(result.is_ok());
        let local_mod = result.unwrap();
        assert_eq!(local_mod.location, valid_path);
    }

    #[test]
    fn test_from_path_invalid_file() {
        let invalid_path = PathBuf::from("invalid_mod.zip");
        let result = LocalMod::from_path(&invalid_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_local_mods() {
        let archive_paths = vec![PathBuf::from("./test/test-mod.zip")];
        let local_mods = LocalMod::load_local_mods(&archive_paths);
        assert!(!local_mods.is_empty());
        assert_eq!(local_mods[0].manifest.name, "test-mod");
    }
}
