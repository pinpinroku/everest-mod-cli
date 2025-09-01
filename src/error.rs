use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModPageUrlParseError {
    #[error("invalid URL: {0}")]
    InvalidUrl(String),

    #[error("unsupported scheme in URL: {0}. Expected 'http' or 'https'")]
    UnsupportedScheme(String),

    #[error("invalid GameBanana URL: {0}. Expected host 'gamebanana.com'")]
    InvalidGameBananaUrl(String),

    #[error("URL cannot be a base URL: {0}")]
    CannotBeBaseUrl(String),

    #[error("invalid path format in URL: {0}. Expected '/mods/<id>'")]
    InvalidPathFormat(String),

    #[error("invalid mod ID: {0}. Expected a positive integer")]
    InvalidModId(String),
}
