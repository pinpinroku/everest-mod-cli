//! Module for handling Everest mod manifest files (`everest.yaml`).
//!
//! This module provides functionality to parse and validate mod manifest files,
//! which are typically written in YAML format. The manifest file contains essential
//! information about the mod, such as its name, version, dependencies, and optional dependencies.
use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur while parsing the manifest file.
#[derive(Debug, Error)]
pub enum ManifestParseError {
    /// The manifest file does not have any mod entries.
    #[error("no mod entries found in the manifest file")]
    NoModEntries,
    /// Failed to parse the manifest file. Invalid YAML syntax.
    #[error(transparent)]
    Parse(#[from] serde_yaml_ng::Error),
}

/// Represents the `everest.yaml` manifest file that defines a mod.
///
/// See [`everest.yaml` Setup Guide](https://github.com/EverestAPI/Resources/wiki/everest.yaml-Setup) for more details.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct ModManifest {
    /// A name of the mod. This is the unique identifier for the mod.
    #[serde(rename = "Name")]
    pub name: String,
    /// A version string of the mod. Not all mods follow semantic versioning. So this is a string.
    #[serde(rename = "Version")]
    pub version: String,
    /// A path to the custom code file.
    #[serde(rename = "DLL")]
    dll: Option<String>,
    /// A list of other mods that this mod depends on.
    #[serde(rename = "Dependencies")]
    pub dependencies: Option<Vec<Dependency>>,
    /// These dependencies get loaded before your mod only if the user already has them installed
    /// and they are the version you specify or higher - they aren't required to play it.
    #[serde(rename = "OptionalDependencies")]
    pub optional_dependencies: Option<Vec<Dependency>>,
}

/// Dependency specification for required mod dependencies.
#[derive(Debug, Default, Deserialize, Serialize, Clone, Hash, PartialEq, Eq)]
pub struct Dependency {
    /// A name of the dependency mod.
    #[serde(rename = "Name")]
    pub name: String,
    /// A version string of the dependency mod.
    #[serde(rename = "Version")]
    pub version: Option<String>,
}

impl ModManifest {
    /// Deserialize an instance of type ModManifest from bytes of YAML text.
    ///
    /// # Errors
    ///
    /// - `Parse`: Failed to parse YAML format. YAML syntax error.
    /// - `NoModEntries`: The manifest file does not have any mod entries.
    pub fn from_slice(yaml_bytes: &[u8]) -> Result<Self, ManifestParseError> {
        // NOTE: We always need first entry from this collection since that is the primal mod, so we use the `VecDeque<T>` here instead of the `Vec<T>`.
        let mut manifest_entries: VecDeque<Self> = serde_yaml_ng::from_slice(yaml_bytes)?;

        match manifest_entries.pop_front() {
            Some(entry) => Ok(entry),
            None => Err(ManifestParseError::NoModEntries),
        }
    }
}

#[cfg(test)]
mod tests_manifest {

    use super::*;

    #[test]
    fn test_from_slice_valid_manifest() -> anyhow::Result<()> {
        let yaml = r#"
        - Name: TestMod
          Version: 1.0.0
        "#;

        let result = ModManifest::from_slice(yaml.as_bytes());
        assert!(result.is_ok());

        let manifest = result?;
        assert_eq!(manifest.name, "TestMod");
        assert_eq!(manifest.version, "1.0.0");
        Ok(())
    }

    #[test]
    fn test_from_slice_invalid_manifest() {
        let yaml = r#"
        TestMod
          Version: 1.0.0
        "#;

        let result = ModManifest::from_slice(yaml.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_from_slice_empty_manifest() {
        let yaml = b"[]";

        let result = ModManifest::from_slice(yaml);
        assert!(result.is_err());
        assert!(
            result
                .err()
                .is_some_and(|e| matches!(e, ManifestParseError::NoModEntries))
        );
    }
}
