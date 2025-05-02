use reqwest::Client;
use std::{collections::HashSet, path::Path};
use tracing::{debug, info, warn};

use crate::{
    download,
    error::Error,
    fileutil::{read_manifest_file_from_zip, replace_home_dir_with_tilde},
    installed_mods::ModManifest,
    mod_registry::{ModRegistryQuery, RemoteModInfo, RemoteModRegistry},
};

/// Install a mod
pub async fn install(
    client: &Client,
    (name, manifest): (&str, &RemoteModInfo),
    mod_registry: &RemoteModRegistry,
    download_dir: &Path,
    installed_mod_names: HashSet<String>,
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

    if let Some(dependencies) = check_dependencies(&download_path)? {
        debug!("Checking for missing dependencies...");
        let missing_dependencies: Vec<_> = dependencies.difference(&installed_mod_names).collect();
        if missing_dependencies.is_empty() {
            info!("You already have all the dependencies required by this mod.");
            return Ok(());
        }

        info!("Start downloading the dependencies...");
        resolve_dependencies(client, mod_registry, download_dir, missing_dependencies).await?;
    }

    Ok(())
}

async fn resolve_dependencies(
    client: &Client,
    mod_registry: &std::collections::HashMap<String, RemoteModInfo>,
    download_dir: &Path,
    missing_dependencies: Vec<&String>,
) -> Result<(), Error> {
    // HACK: Make this concurrently
    for dependency in missing_dependencies {
        if let Some((mod_name, manifest)) = mod_registry.get_mod_info_by_name(dependency) {
            debug!("Manifest of dependency: {}\n{:#?}", mod_name, manifest);
            download::download_mod(
                client,
                mod_name,
                &manifest.download_url,
                &manifest.checksums,
                download_dir,
            )
            .await?;
        } else {
            warn!(
                "Could not find information about the mod '{}'.\n\
                    The modder might have misspelled the name.",
                dependency
            );
        }
    }
    Ok(())
}

/// Check for dependencies, if found return `HashSet<String>`, otherwise return `None`.
fn check_dependencies(download_path: &Path) -> Result<Option<HashSet<String>>, Error> {
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
            .map(|dependency| dependency.name)
            .collect::<HashSet<String>>();
        debug!("Filtered dependencies: {:#?}", filtered_deps);
        Ok(Some(filtered_deps))
    } else {
        warn!("No dependencies found. This is weird. Even 'Everest' is not listed.");
        Ok(None)
    }
}
