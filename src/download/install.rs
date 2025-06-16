use std::collections::HashSet;

use crate::{
    dependency::{DependencyGraph, ModDependencyQuery},
    mod_registry::{RemoteModInfo, RemoteModRegistry},
};

pub fn check_dependencies(
    mod_name: &str,
    mod_registry: &RemoteModRegistry,
    dependency_graph: &DependencyGraph,
    installed_mod_names: &HashSet<String>,
) -> Vec<(String, RemoteModInfo)> {
    // Collects required dependencies for the mod including the mod itself
    let dependencies = dependency_graph.collect_all_dependencies_bfs(mod_name);

    // Filters out missing dependencies
    let missing_deps = dependencies
        .difference(installed_mod_names)
        .collect::<Vec<_>>();
    tracing::debug!("Missing dependencies are found: {:?}", missing_deps);

    missing_deps
        .iter()
        .filter_map(|name| {
            let name = (*name).clone();
            if let Some(remote_mod) = mod_registry.get(&name) {
                tracing::info!(
                    "Dependency [{}] is available: {}",
                    name,
                    remote_mod.download_url
                );
                Some((name, remote_mod.to_owned()))
            } else {
                tracing::warn!("Dependency [{}] is not available in the registry", name);
                None
            }
        })
        .collect::<Vec<_>>()
}
