use std::borrow::Cow;

use reqwest::Url;

use crate::error::ModPageUrlParseError;

/// Parses the given url string, converts it to the mod ID.
///
/// # Errors
/// Returns an error if the URL is invalid, has an unsupported scheme,
/// or does not match the expected GameBanana mod page format.
pub fn parse_mod_page_url(page_url_str: &str) -> Result<u32, ModPageUrlParseError> {
    let page_url = Url::parse(page_url_str)
        .map_err(|_| ModPageUrlParseError::InvalidUrl(page_url_str.to_owned()))?;

    // Check scheme
    match page_url.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(ModPageUrlParseError::UnsupportedScheme(
                page_url_str.to_owned(),
            ));
        }
    }

    // Check host
    if page_url.host_str() != Some("gamebanana.com") {
        return Err(ModPageUrlParseError::InvalidGameBananaUrl(page_url.clone()));
    }

    // Check path segments
    let mut segments = page_url
        .path_segments()
        .ok_or_else(|| ModPageUrlParseError::InvalidGameBananaUrl(page_url.clone()))?;

    // Expected path: /mods/12345
    match (segments.next(), segments.next()) {
        (Some("mods"), Some(id_str)) => {
            let id = id_str
                .parse::<u32>()
                .map_err(|_| ModPageUrlParseError::InvalidModId(id_str.to_owned()))?;
            Ok(id)
        }
        _ => Err(ModPageUrlParseError::InvalidGameBananaUrl(page_url.clone())),
    }
}

/// Returns sanitized mod name or "unnamed" if the given mod name is empty.
///
/// This function replaces any invalid characters with underscores, trims whitespace,
/// and ensures the resulting string does not exceed 255 characters.
pub fn sanitize(mod_name: &str) -> Cow<'_, str> {
    const BAD_CHARS: [char; 6] = ['/', '\\', '*', '?', ':', ';'];

    let trimmed = mod_name.trim();
    let without_dot = trimmed.strip_prefix('.').unwrap_or(trimmed);

    let mut changed = false;
    let mut result = String::with_capacity(without_dot.len());

    for c in without_dot
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
    {
        let replacement = match c {
            '\r' | '\n' | '\0' => {
                changed = true;
                continue;
            }
            c if BAD_CHARS.contains(&c) => {
                changed = true;
                '_'
            }
            c => c,
        };
        result.push(replacement);
    }

    if result.len() > 255 {
        result.truncate(255);
        changed = true;
    }

    if result.is_empty() {
        Cow::Borrowed("unnamed")
    } else if !changed && result == mod_name {
        Cow::Borrowed(mod_name)
    } else {
        Cow::Owned(result)
    }
}

#[cfg(test)]
mod tests_page_url {
    use super::*;

    #[test]
    fn test_valid_url() {
        let url = "https://gamebanana.com/mods/12345";
        assert_eq!(parse_mod_page_url(url).unwrap(), 12345);
    }

    #[test]
    fn test_invalid_scheme() {
        let url = "ftp://gamebanana.com/mods/12345";
        assert!(parse_mod_page_url(url).is_err());
    }

    #[test]
    fn test_invalid_host() {
        let url = "https://example.com/mods/12345";
        assert!(parse_mod_page_url(url).is_err());
    }

    #[test]
    fn test_missing_id() {
        let url = "https://gamebanana.com/mods/";
        assert!(parse_mod_page_url(url).is_err());
    }

    #[test]
    fn test_non_numeric_id() {
        let url = "https://gamebanana.com/mods/abc";
        assert!(parse_mod_page_url(url).is_err());
    }
}

#[cfg(test)]
mod tests_sanitize {
    use super::*;

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize(""), Cow::Borrowed("unnamed"));
    }

    #[test]
    fn test_sanitize_with_bad_chars() {
        assert_eq!(
            sanitize("Mod/Name*With?Bad:Chars;"),
            Cow::Owned::<str>("Mod_Name_With_Bad_Chars_".to_string())
        );
    }

    #[test]
    fn test_sanitize_with_whitespace() {
        assert_eq!(
            sanitize("  Mod Name  "),
            Cow::Owned::<str>("Mod Name".to_string())
        );
    }

    #[test]
    fn test_sanitize_long_name() {
        let long_name = "a".repeat(300);
        assert_eq!(sanitize(&long_name).len(), 255);
    }
}
