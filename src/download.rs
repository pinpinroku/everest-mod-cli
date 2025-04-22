//! # Download Module
//!
//! This module provides functionality for downloading mods and working
//! with mod metadata. It includes the primary structures and functions
//! for fetching the mod registry, downloading mod files, verifying checksums,
//! and determining appropriate filenames based on URL metadata and HTTP headers.
//!
//! ## Key Components
//!
//! - **ModDownloader**: A struct that manages mod downloads and registry fetching.
//! - **download_mod**: An async method that downloads a mod file, shows a progress bar,
//!   computes an xxHash checksum, and verifies the integrity of the downloaded file.
//! - **fetch_mod_registry**: Fetches the remote mod registry as raw bytes.
//! - **util Module**: Contains utility functions such as `determine_filename`,
//!   which extracts or generates a filename based on URL and ETag header metadata.
//!
//! ## Usage
//!
//! The module can be used to fetch and download mods from a remote registry,
//! verify file integrity using checksums, and determine a filename for storage.
//!
//! For example:
//!
//! ```rust
//! # async fn example() -> Result<(), Error> {
//!     let download_dir = Path::new("/path/to/downloads");
//!     let mod_downloader = ModDownloader::new(download_dir);
//!
//!     // Fetch the mod registry
//!     let registry_data = mod_downloader.fetch_mod_registry().await?;
//!
//!     // Download a mod file
//!     let mod_url = "https://example.com/modfile.zip";
//!     let mod_name = "ExampleMod";
//!     let expected_hashes = vec!["abcdef1234567890".to_string()];
//!     mod_downloader.download_mod(mod_url, mod_name, &expected_hashes).await?;
//!
//!     Ok(())
//!  }
//! ```
//!
//! Ensure that necessary dependencies such as `reqwest`, `tokio`, and `uuid` are included
//! in your Cargo.toml.
use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::{fs, io::AsyncWriteExt};
use tracing::info;
use xxhash_rust::xxh64::Xxh64;

use crate::{constant::MOD_REGISTRY_URL, error::Error};

/// Manages mod downloads and registry fetching.
#[derive(Debug, Clone)]
pub struct ModDownloader {
    client: Client,
    registry_url: String,
    download_dir: PathBuf,
}

impl ModDownloader {
    /// Creates a new `ModDownloader` with the specified download directory.
    ///
    /// # Parameters
    /// - `download_dir`: The directory where downloaded mods will be stored.
    ///
    /// # Returns
    /// A new instance of `ModDownloader`.
    pub fn new(download_dir: &Path) -> Self {
        Self {
            client: Client::new(),
            registry_url: String::from(MOD_REGISTRY_URL),
            download_dir: download_dir.to_path_buf(),
        }
    }

    /// Fetches the remote mod registry.
    ///
    /// # Returns
    /// - `Ok(Bytes)`: The raw bytes of the registry file upon success.
    /// - `Err(Error)`: An error if the request or parsing fails.
    pub async fn fetch_mod_registry(&self) -> Result<Bytes, Error> {
        info!("Fetching remote mod registry...");
        let response = self.client.get(&self.registry_url).send().await?;
        let yaml_data = response.bytes().await?;
        Ok(yaml_data)
    }

    /// Downloads a mod file from the given URL, saves it locally, and verifies its integrity.
    ///
    /// # Parameters
    /// - `url`: The URL to download the mod from.
    /// - `name`: The mod's name (used for logging and display).
    /// - `expected_hash`: Slice of acceptable xxHash checksum strings (in hexadecimal).
    ///
    /// # Behavior
    /// 1. Sends an HTTP GET request to download the file.
    /// 2. Determines the filename from the response and creates a local file.
    /// 3. Streams the file to disk, updating a progress bar and computing its xxHash.
    /// 4. Verifies the computed checksum against the provided expected hash values.
    ///    - If verification fails, the file is removed and an `InvalidChecksum` error is returned.
    ///
    /// # Returns
    /// Returns `Ok(())` if the download and checksum verification succeed, otherwise returns an appropriate `Error`.
    pub async fn download_mod(
        &self,
        url: &str,
        name: &str,
        expected_hash: &[String],
    ) -> Result<(), Error> {
        println!("\nDownloading {}:", name);

        let response = self.client.get(url).send().await?.error_for_status()?;
        info!("Status code: {:#?}", response.status());

        let filename = util::determine_filename(&response)?;
        let download_path = self.download_dir.join(filename);
        info!("Destination: {:#?}", download_path);

        let total_size = response.content_length().unwrap_or(0);
        info!("Total file size: {}", total_size);

        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        let mut stream = response.bytes_stream();

        let mut hasher = Xxh64::new(0);
        let mut file = fs::File::create(&download_path).await?;
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            hasher.update(&chunk);
            let new = std::cmp::min(downloaded + (chunk.len() as u64), total_size);
            downloaded = new;
            pb.set_position(new);
        }

        pb.finish_with_message("Download complete");

        let hash = hasher.digest();
        let hash_str = format!("{:016x}", hash);
        info!(
            "Xxhash in u64: {:#?}, formatted string: {:#?}",
            hash, hash_str
        );

        // Verify checksum
        println!("\n🔍 Verifying checksum of the mod '{}'", name);
        if expected_hash.contains(&hash_str) {
            println!("✅ Checksum verified!");
        } else {
            println!("❌ Checksum verification failed!");
            fs::remove_file(&download_path).await?;
            println!("[Cleanup] Downloaded file removed 🗑️");
            return Err(Error::InvalidChecksum {
                file: download_path,
                computed: hash_str,
                expected: expected_hash.to_vec(),
            });
        }

        Ok(())
    }
}

/// Utility functions for determining filenames and handling mod download metadata.
mod util {
    use super::*;
    use reqwest::{Response, Url};
    use uuid::Uuid;

    /// Determines the most appropriate filename for a downloaded mod using the URL and metadata.
    ///
    /// It first attempts to extract the filename from the URL. If that fails, it tries to extract
    /// the filename from the response’s ETag header. If both methods fail, it generates a random filename.
    ///
    /// # Parameters
    /// - `response`: The HTTP response from which to extract metadata.
    ///
    /// # Returns
    /// - `Ok(String)`: The determined filename.
    /// - `Err(Error)`: An error if filename extraction fails.
    pub fn determine_filename(response: &Response) -> Result<String, Error> {
        let filename_from_url = extract_filename_from_url(response.url());
        let filename_from_etag = extract_filename_from_etag(response);
        let mod_filename = filename_from_url
            .or(filename_from_etag)
            .unwrap_or_else(|| format!("unknown-mod_{}.zip", Uuid::new_v4()));

        Ok(mod_filename)
    }

    /// Extracts a filename from the last segment of a URL path.
    ///
    /// # Parameters
    /// - `url`: The URL from which to parse the filename.
    ///
    /// # Returns
    /// An optional filename extracted from the URL.
    fn extract_filename_from_url(url: &Url) -> Option<String> {
        url.path_segments()
            .and_then(|mut segments| segments.next_back().filter(|&segment| !segment.is_empty()))
            .map(String::from)
    }

    /// Extracts a filename from the ETag header, appending a `.zip` extension.
    ///
    /// # Parameters
    /// - `response`: The HTTP response containing headers.
    ///
    /// # Returns
    /// An optional formatted filename based on the ETag value.
    fn extract_filename_from_etag(response: &Response) -> Option<String> {
        response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|etag_value| etag_value.to_str().ok())
            .map(|etag| etag.trim_matches('"').to_string())
            .map(|etag| format!("{}.zip", etag))
    }
}
