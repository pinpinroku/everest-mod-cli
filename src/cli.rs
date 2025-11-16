use std::{num::ParseIntError, path::PathBuf};

use clap::{Args, Parser, Subcommand};

/// The main CLI structure for the Everest Mod CLI application
#[derive(Debug, Parser)]
#[command(version, about = "Mod management tool for Celeste", long_about = None)]
pub struct Cli {
    /// Directory where mods are stored. This option applies to all commands
    #[arg(short = 'd', long = "mods-dir", value_name = "DIR")]
    pub mods_directory: Option<PathBuf>,

    /// Priority of the mirror list separated by commas
    #[arg(
        short = 'm',
        long = "mirror-priority",
        value_name = "MIRROR",
        long_help = "Priority of the mirror list separated by commas (e.g., \"wegfan,jade,gb,otobot\").
        This option only applies to the `install` and the `update` commands,

        * gb     => 'Default GameBanana Server (United States)',
        * jade   => 'Germany',
        * wegfan => 'China',
        * otobot => 'North America',

        If the download from the current server fails, the application will
        automatically fall back to the next server in the priority list to
        retry the download. You can also restrict the fallback servers by
        providing a comma-separated list (e.g., \"otobot,jade\"), which will
        limit the retries to only those specified servers.",
        default_value = "otobot,gb,jade,wegfan"
    )]
    pub mirror_preferences: String,

    /// Verbose mode: Write verbose logs to the file
    #[arg(short, long)]
    pub verbose: bool,

    /// The subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// The set of available subcommands for the Everest Mod CLI
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Install a mod using the URL
    Install(InstallArgs),
    /// List installed mods
    List,
    /// Show detailed information about an installed mod
    Show(ShowArgs),
    /// Check for updates
    Update(UpdateArgs),
}

/// Arguments for the `install` subcommand
#[derive(Debug, Args)]
pub struct InstallArgs {
    /// The URL of the page where the mod is featured on the GameBanana
    pub mod_page_url: String,
}

/// Arguments for the `show` subcommand
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// The name of the mod to show details for
    pub name: String,
}

/// Arguments for the `update` subcommand
#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Install available updates
    #[arg(long, action)]
    pub install: bool,
}

/// A valid prefix for the mod page URL
const VALID_MOD_PAGE_URL_PREFIX: &str = "https://gamebanana.com/mods/";

/// An error can be occured when trying to extract an ID from an URL
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum IdExtractionError {
    #[error("'{url}' does not have valid prefix (expected: '{VALID_MOD_PAGE_URL_PREFIX}')")]
    InvalidPrefix { url: String },
    #[error("no valid ID segment in given URL")]
    NoIdSegment,
}

/// Extracts an ID segment from given URL string.
pub fn extract_id(url: &str) -> Result<&str, IdExtractionError> {
    let id_str = url.strip_prefix(VALID_MOD_PAGE_URL_PREFIX);
    match id_str {
        Some(id) if !id.is_empty() => Ok(id),
        Some(_) => Err(IdExtractionError::NoIdSegment),
        None => Err(IdExtractionError::InvalidPrefix {
            url: url.to_string(),
        }),
    }
}

/// Parses given string into an integer.
pub fn parse_id(id_str: &str) -> Result<u32, ParseIntError> {
    id_str
        .parse::<u32>()
        .inspect(|id| tracing::info!("parsed id: {}", id))
        .inspect_err(|err| tracing::error!("failed to parse '{}' cause: {}", id_str, err))
}

#[cfg(test)]
mod tests_id_extraction {
    use super::*;

    #[test]
    fn test_extract_id_valid_numeric() {
        let url = "https://gamebanana.com/mods/123456";
        assert_eq!(extract_id(url).unwrap(), "123456");
    }

    #[test]
    fn test_extract_id_valid_with_trailing_path() {
        let url = "https://gamebanana.com/mods/123456/download";
        assert_eq!(extract_id(url).unwrap(), "123456/download");
    }

    #[test]
    fn test_extract_id_valid_with_query_params() {
        let url = "https://gamebanana.com/mods/123456?tab=comments";
        assert_eq!(extract_id(url).unwrap(), "123456?tab=comments");
    }

    #[test]
    fn test_extract_id_valid_alphanumeric() {
        let url = "https://gamebanana.com/mods/abc123def";
        assert_eq!(extract_id(url).unwrap(), "abc123def");
    }

    #[test]
    fn test_extract_id_empty_id_segment() {
        let url = "https://gamebanana.com/mods/";
        let result = extract_id(url);
        assert_eq!(result, Err(IdExtractionError::NoIdSegment));
    }

    #[test]
    fn test_extract_id_invalid_prefix_different_domain() {
        let url = "https://example.com/mods/123456";
        let result = extract_id(url);
        assert_eq!(
            result,
            Err(IdExtractionError::InvalidPrefix {
                url: url.to_string()
            })
        );
    }

    #[test]
    fn test_extract_id_invalid_prefix_different_path() {
        let url = "https://gamebanana.com/mmdl/123456";
        let result = extract_id(url);
        assert_eq!(
            result,
            Err(IdExtractionError::InvalidPrefix {
                url: url.to_string()
            })
        );
    }

    #[test]
    fn test_extract_id_invalid_prefix_missing_trailing_slash() {
        let url = "https://gamebanana.com/mods123456";
        let result = extract_id(url);
        assert_eq!(
            result,
            Err(IdExtractionError::InvalidPrefix {
                url: url.to_string()
            })
        );
    }

    #[test]
    fn test_extract_id_invalid_prefix_http_instead_of_https() {
        let url = "http://gamebanana.com/mods/123456";
        let result = extract_id(url);
        assert_eq!(
            result,
            Err(IdExtractionError::InvalidPrefix {
                url: url.to_string()
            })
        );
    }

    #[test]
    fn test_extract_id_invalid_prefix_with_subdomain() {
        let url = "https://www.gamebanana.com/mods/123456";
        let result = extract_id(url);
        assert_eq!(
            result,
            Err(IdExtractionError::InvalidPrefix {
                url: url.to_string()
            })
        );
    }

    #[test]
    fn test_extract_id_empty_string() {
        let url = "";
        let result = extract_id(url);
        assert_eq!(
            result,
            Err(IdExtractionError::InvalidPrefix {
                url: url.to_string()
            })
        );
    }

    #[test]
    fn test_extract_id_valid_with_fragment() {
        let url = "https://gamebanana.com/mods/123456#description";
        assert_eq!(extract_id(url).unwrap(), "123456#description");
    }
}
