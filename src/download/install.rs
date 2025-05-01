use reqwest::Client;
use std::path::Path;
use tracing::{debug, info, warn};

use crate::{
    download,
    error::Error,
    fileutil::{read_manifest_file_from_zip, replace_home_dir_with_tilde},
    installed_mods::{Dependency, ModManifest},
    mod_registry::{ModRegistryQuery, RemoteModInfo, RemoteModRegistry},
};

/// Install a mod
pub async fn install(
    client: &Client,
    (name, manifest): (&str, &RemoteModInfo),
    mod_registry: &RemoteModRegistry,
    download_dir: &Path,
    installed_mod_names: Vec<String>,
) -> Result<(), Error> {
    let download_path = download::download_mod(
        client,
        name,
        &manifest.download_url,
        &manifest.checksums,
        download_dir,
    )
    .await?;

    info!(
        "[{}] is now installed in {}.",
        name,
        replace_home_dir_with_tilde(&download_path)
    );

    if let Some(mut deps) = check_dependencies(&download_path)? {
        debug!("Filtering out already installed dependencies.");
        deps.retain(|dep| !installed_mod_names.contains(&dep.name));
        if !deps.is_empty() {
            info!("Start downloading dependencies...");
            // HACK: Extract this into function to handle multiple downloads concurrently
            for dep in deps {
                if let Some((name, manifest)) = mod_registry.get_mod_info_by_name(&dep.name) {
                    debug!("Manifest of dependency: {}\n{:#?}", name, manifest);
                    download::download_mod(
                        client,
                        name,
                        &manifest.download_url,
                        &manifest.checksums,
                        download_dir,
                    )
                    .await?;
                }
            }
        } else {
            info!("You already have all the dependencies required by this mod.");
        }
    }

    Ok(())
}

/// Check for dependencies, if found return `Vec<Dependency>`, otherwise return `None`.
fn check_dependencies(download_path: &Path) -> Result<Option<Vec<Dependency>>, Error> {
    info!("Checking for dependencies...");
    // Attempt to read the manifest file. If it doesn't exist, return an error.
    let buffer = read_manifest_file_from_zip(download_path)?
        .ok_or_else(|| Error::MissingManifestFile(download_path.to_path_buf()))?;

    // Parse the manifest file
    let manifest = ModManifest::parse_mod_manifest_from_yaml(&buffer)?;
    debug!("Manifest content: {:#?}", manifest);

    // Retrieve dependencies if available, filtering out "Everest" and "EverestCore"
    if let Some(dependencies) = manifest.dependencies {
        let filtered_deps = dependencies
            .into_iter()
            // NOTE: "Everest" and "EverestCore (deprecated)" are primal dependencies, so there is no need to download them
            .filter(|dependency| !matches!(dependency.name.as_str(), "Everest" | "EverestCore"))
            .collect::<Vec<Dependency>>();
        debug!("Filtered dependencies: {:#?}", filtered_deps);
        Ok(Some(filtered_deps))
    } else {
        warn!("No dependencies found. This is weird. Even 'Everest' is not listed.");
        Ok(None)
    }
}
