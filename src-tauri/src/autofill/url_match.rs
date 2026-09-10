use tauri::Url;

use super::protocol::ErrorCode;

/// A parsed hostname, never a user-entered domain substring. No DNS lookup.
pub(super) struct HttpsHost(String);

fn canonical_autofill_host(hostname: &str) -> Result<String, ErrorCode> {
    let lowercase = hostname.to_ascii_lowercase();
    let without_trailing_dot = lowercase.strip_suffix('.').unwrap_or(&lowercase);
    // The only automatic hostname equivalence is one leading `www.` label.
    // All other labels remain intact; comparison remains whole-string equality.
    let canonical = without_trailing_dot
        .strip_prefix("www.")
        .unwrap_or(without_trailing_dot);
    if canonical.is_empty() || canonical.split('.').any(str::is_empty) {
        return Err(ErrorCode::UnsupportedUrl);
    }
    Ok(canonical.to_owned())
}

impl HttpsHost {
    pub(super) fn parse(value: &str) -> Result<Self, ErrorCode> {
        // Reject URL-parser repairs of missing authority delimiters, backslashes,
        // whitespace and control characters. Stored URLs are never guessed.
        let (scheme, authority_and_path) =
            value.split_once("://").ok_or(ErrorCode::UnsupportedUrl)?;
        if !scheme.eq_ignore_ascii_case("https")
            || authority_and_path.starts_with('/')
            || value
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
        {
            return Err(ErrorCode::UnsupportedUrl);
        }
        let url = Url::parse(value).map_err(|_| ErrorCode::UnsupportedUrl)?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(ErrorCode::UnsupportedUrl);
        }
        let host = url.host_str().ok_or(ErrorCode::UnsupportedUrl)?;
        Ok(Self(canonical_autofill_host(host)?))
    }

    pub(super) fn canonical(&self) -> &str {
        &self.0
    }

    pub(super) fn matches_saved(
        &self,
        website: Option<&str>,
        allowed_login_hosts: &[String],
    ) -> bool {
        let primary_matches = website
            .and_then(|website| Self::parse(website).ok())
            .is_some_and(|saved| self.0 == saved.0);
        primary_matches
            || allowed_login_hosts.iter().any(|allowed| {
                Self::parse_allowed_host(allowed).is_some_and(|saved| self.0 == saved.0)
            })
    }

    fn parse_allowed_host(value: &str) -> Option<Self> {
        if value.is_empty()
            || value.contains(['*', '/', '\\', ':', '?', '#', '@', '[', ']'])
            || value
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
        {
            return None;
        }
        Self::parse(&format!("https://{value}/")).ok()
    }
}
