use std::sync::Arc;

use crate::{
    local::{Generatable, LocalMod},
    mod_registry::{RemoteModInfo, RemoteModRegistry},
};

/// Checks for updates to the local mods against the remote mod registry.
pub fn check_updates(
    local_mods: &[LocalMod],
    mod_registry: Arc<RemoteModRegistry>,
) -> Vec<(String, RemoteModInfo)> {
    use rayon::prelude::*;

    local_mods
        .par_iter()
        .filter_map(|local_mod| {
            let name = &local_mod.manifest.name;
            let remote_mod = mod_registry.get(name)?;

            let local_hash = match local_mod.checksum() {
                Ok(hash) => hash,
                Err(e) => {
                    tracing::warn!("Failed to compute checksum for {}: {}", name, e);
                    return None;
                }
            };

            if remote_mod.has_matching_hash(local_hash) {
                None
            } else {
                println!(
                    "Update available for '{}': {} -> {}",
                    name, local_mod.manifest.version, remote_mod.version
                );
                Some((name.clone(), remote_mod.clone()))
            }
        })
        .collect()
}
