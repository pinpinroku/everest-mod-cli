use std::collections::HashMap;

use anyhow::Result;
use reqwest::Client;
use serde::de::DeserializeOwned;

use crate::{
    dependency::{DependencyGraph, DependencyInfo, ModDependencyQuery},
    mod_registry::{ModRegistryQuery, RemoteModInfo, RemoteModRegistry},
};

/// Fetches the remote data from the given URL and parses it into the specified type.
pub async fn fetch_remote_data<T>(url: &str, client: &Client) -> Result<T>
where
    T: DeserializeOwned,
{
    let response = client.get(url).send().await?.error_for_status()?;
    tracing::info!("'{}' -> Status: {}", url, response.status());

    let bytes = response.bytes().await?;
    let data = serde_yaml_ng::from_slice::<T>(&bytes)?;

    Ok(data)
}

/// Fetches online database.
pub async fn fetch_online_database(
    client: &Client,
) -> Result<(
    HashMap<String, RemoteModInfo>,
    HashMap<String, DependencyInfo>,
)> {
    tracing::info!("Fetching mod registry and dependency graph from remote server...");
    let spinner = crate::download::pb_style::create_spinner();
    let (mod_registry, dependency_graph) = tokio::try_join!(
        RemoteModRegistry::fetch(client),
        DependencyGraph::fetch(client)
    )?;
    spinner.finish_and_clear();

    tracing::info!("Successfully fetched mod registry and dependency graph");
    tracing::debug!("Fetched mod registry with {} entries", mod_registry.len());
    tracing::debug!(
        "Fetched dependency graph with {} entries",
        dependency_graph.len()
    );

    Ok((mod_registry, dependency_graph))
}
