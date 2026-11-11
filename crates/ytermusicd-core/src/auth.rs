use crate::paths::Paths;
use serde::Serialize;
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;
use ytpapi2::{COOKIE, HeaderMap, HeaderValue, USER_AGENT};

const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:108.0) Gecko/20100101 Firefox/108.0";

/// Validated YouTube Music credentials.
#[derive(Clone, PartialEq, Eq)]
pub struct Auth {
    cookie: String,
    user_agent: String,
    sapisid: String,
    account_id: Option<String>,
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Auth")
            .field("cookie", &"<redacted>")
            .field("user_agent", &self.user_agent)
            .field("sapisid", &"<redacted>")
            .field(
                "account_id",
                &self.account_id.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl Auth {
    /// Load and validate a `headers.txt` file. The optional `account_id.txt`
    /// is read from the same directory.
    pub fn from_path(path: &Path) -> Result<Self, AuthError> {
        let content = fs::read_to_string(path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                AuthError::NotFound {
                    path: path.to_path_buf(),
                }
            } else {
                AuthError::Io {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?;
        let account_id = read_account_id(path);
        Self::parse_with_path(path, &content, account_id)
    }

    /// Build credentials from a raw cookie string (used by the TUI's browser
    /// cookie import). A default User-Agent is used.
    pub fn from_cookie(
        cookie: impl Into<String>,
        account_id: Option<String>,
    ) -> Result<Self, AuthError> {
        let content = format!("Cookie: {}\n", cookie.into());
        Self::parse_with_path(Path::new("<cookies>"), &content, account_id)
    }

    fn parse_with_path(
        path: &Path,
        content: &str,
        account_id: Option<String>,
    ) -> Result<Self, AuthError> {
        let mut cookie = None;
        let mut user_agent = None;

        for (index, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((name, value)) = line.split_once(':') else {
                return Err(AuthError::MalformedHeader {
                    path: path.to_path_buf(),
                    line: index + 1,
                });
            };
            let name = name.trim();
            let value = value.trim();
            if name.eq_ignore_ascii_case("cookie") {
                cookie = Some(value.to_string());
            } else if name.eq_ignore_ascii_case("user-agent") {
                user_agent = Some(value.to_string());
            } else {
                log::debug!("ignoring unknown header `{name}` in {}", path.display());
            }
        }

        let cookie = cookie.ok_or_else(|| AuthError::MissingCookie {
            path: path.to_path_buf(),
        })?;
        HeaderValue::from_str(&cookie).map_err(|error| AuthError::InvalidCookie {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

        let sapisid =
            cookie_value(&cookie, "SAPISID").ok_or_else(|| AuthError::MissingSapisid {
                path: path.to_path_buf(),
            })?;

        let user_agent = user_agent.unwrap_or_else(|| {
            log::warn!(
                "no User-Agent in {}; using a default browser User-Agent",
                path.display()
            );
            DEFAULT_USER_AGENT.to_string()
        });
        HeaderValue::from_str(&user_agent).map_err(|error| AuthError::InvalidUserAgent {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;

        Ok(Self {
            cookie,
            user_agent,
            sapisid,
            account_id,
        })
    }

    pub fn cookie(&self) -> &str {
        &self.cookie
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub fn sapisid(&self) -> &str {
        &self.sapisid
    }

    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    /// Build the request headers expected by `ytpapi2`.
    pub fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&self.cookie).expect("cookie was validated"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.user_agent).expect("user agent was validated"),
        );
        headers
    }

    /// Construct a YouTube Music client with these credentials.
    pub async fn create_client(
        &self,
    ) -> Result<ytpapi2::YoutubeMusicInstance, ytpapi2::YoutubeMusicError> {
        ytpapi2::YoutubeMusicInstance::new(self.headers(), self.account_id.clone()).await
    }
}

fn cookie_value(cookie: &str, name: &str) -> Option<String> {
    cookie.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        if key.trim() == name {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

fn read_account_id(headers: &Path) -> Option<String> {
    let path = Paths::account_id_file_near(headers);
    match fs::read_to_string(path) {
        Ok(id) => {
            let id = id.trim();
            if id.is_empty() {
                None
            } else {
                Some(id.to_string())
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            log::warn!(
                "failed to read account_id next to {}: {error}",
                headers.display()
            );
            None
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("headers file not found: {}", path.display())]
    NotFound { path: PathBuf },
    #[error("failed to read headers {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("malformed header on line {line} of {}: expected `Name: value`", path.display())]
    MalformedHeader { path: PathBuf, line: usize },
    #[error("missing `Cookie` header in {}", path.display())]
    MissingCookie { path: PathBuf },
    #[error("cookie in {} contains invalid characters: {message}", path.display())]
    InvalidCookie { path: PathBuf, message: String },
    #[error("cookie in {} does not contain SAPISID", path.display())]
    MissingSapisid { path: PathBuf },
    #[error("invalid User-Agent in {}: {message}", path.display())]
    InvalidUserAgent { path: PathBuf, message: String },
}

impl AuthError {
    pub fn path(&self) -> &Path {
        match self {
            Self::NotFound { path }
            | Self::Io { path, .. }
            | Self::MalformedHeader { path, .. }
            | Self::MissingCookie { path }
            | Self::InvalidCookie { path, .. }
            | Self::MissingSapisid { path }
            | Self::InvalidUserAgent { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedAuth {
    pub auth: Auth,
    pub source: PathBuf,
    pub legacy: bool,
}

/// Resolve auth in this order: explicit path, primary config dir, legacy
/// `ytermusic` config dir.
pub fn resolve_auth(paths: &Paths, requested: Option<&Path>) -> Result<LoadedAuth, AuthError> {
    if let Some(path) = requested {
        let auth = Auth::from_path(path)?;
        return Ok(LoadedAuth {
            auth,
            source: path.to_path_buf(),
            legacy: false,
        });
    }

    let primary = paths.headers_file();
    if primary.exists() {
        let auth = Auth::from_path(&primary)?;
        return Ok(LoadedAuth {
            auth,
            source: primary,
            legacy: false,
        });
    }

    let legacy = paths.legacy_headers_file();
    if legacy.exists() {
        log::warn!(
            "using legacy headers file {}; move it to {}",
            legacy.display(),
            primary.display()
        );
        let auth = Auth::from_path(&legacy)?;
        return Ok(LoadedAuth {
            auth,
            source: legacy,
            legacy: true,
        });
    }

    Err(AuthError::NotFound { path: primary })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Ok,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthStatus {
    pub state: AuthState,
    pub source: Option<PathBuf>,
    pub message: Option<String>,
}

impl AuthStatus {
    pub fn ok(source: impl Into<PathBuf>) -> Self {
        Self {
            state: AuthState::Ok,
            source: Some(source.into()),
            message: None,
        }
    }

    pub fn from_error(error: &AuthError) -> Self {
        let state = match error {
            AuthError::NotFound { .. } => AuthState::Missing,
            _ => AuthState::Invalid,
        };
        Self {
            state,
            source: Some(error.path().to_path_buf()),
            message: Some(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn test_paths(root: &Path) -> Paths {
        Paths {
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            data_dir: root.join("data"),
            state_dir: root.join("state"),
            legacy_config_dir: root.join("legacy-config"),
            legacy_cache_dir: root.join("legacy-cache"),
            legacy_data_dir: root.join("legacy-data"),
            legacy_state_dir: root.join("legacy-state"),
        }
    }

    fn write_headers(dir: &Path, content: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join("headers.txt");
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn loads_valid_headers_and_account_id() {
        let dir = tempdir().unwrap();
        let path = write_headers(
            dir.path(),
            "Cookie: SAPISID=secret; other=value\nUser-Agent: test-agent/1.0\n",
        );
        fs::write(dir.path().join("account_id.txt"), "brand-123\n").unwrap();

        let auth = Auth::from_path(&path).unwrap();
        assert_eq!(auth.cookie(), "SAPISID=secret; other=value");
        assert_eq!(auth.user_agent(), "test-agent/1.0");
        assert_eq!(auth.sapisid(), "secret");
        assert_eq!(auth.account_id(), Some("brand-123"));
    }

    #[test]
    fn missing_file_is_reported() {
        let dir = tempdir().unwrap();
        let error = Auth::from_path(&dir.path().join("headers.txt")).unwrap_err();
        assert!(matches!(error, AuthError::NotFound { .. }));
        assert!(error.to_string().contains("headers.txt"));
    }

    #[test]
    fn malformed_header_is_reported() {
        let dir = tempdir().unwrap();
        let path = write_headers(dir.path(), "Cookie SAPISID=secret\n");
        let error = Auth::from_path(&path).unwrap_err();
        assert!(matches!(error, AuthError::MalformedHeader { line: 1, .. }));
    }

    #[test]
    fn missing_cookie_is_reported() {
        let dir = tempdir().unwrap();
        let path = write_headers(dir.path(), "User-Agent: test-agent\n");
        let error = Auth::from_path(&path).unwrap_err();
        assert!(matches!(error, AuthError::MissingCookie { .. }));
    }

    #[test]
    fn missing_sapisid_is_reported() {
        let dir = tempdir().unwrap();
        let path = write_headers(dir.path(), "Cookie: other=value\n");
        let error = Auth::from_path(&path).unwrap_err();
        assert!(matches!(error, AuthError::MissingSapisid { .. }));
    }

    #[test]
    fn default_user_agent_is_used_when_missing() {
        let dir = tempdir().unwrap();
        let path = write_headers(dir.path(), "Cookie: SAPISID=secret\n");
        let auth = Auth::from_path(&path).unwrap();
        assert_eq!(auth.user_agent(), DEFAULT_USER_AGENT);
    }

    #[test]
    fn debug_output_redacts_secrets() {
        let auth = Auth {
            cookie: "SAPISID=super-secret".to_string(),
            user_agent: "test-agent".to_string(),
            sapisid: "super-secret".to_string(),
            account_id: Some("brand-123".to_string()),
        };
        let debug = format!("{auth:?}");
        assert!(!debug.contains("super-secret"));
        assert!(!debug.contains("brand-123"));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn resolve_prefers_primary_over_legacy() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path());
        let primary = write_headers(
            &paths.config_dir,
            "Cookie: SAPISID=primary\nUser-Agent: test\n",
        );
        write_headers(
            &paths.legacy_config_dir,
            "Cookie: SAPISID=legacy\nUser-Agent: test\n",
        );

        let loaded = resolve_auth(&paths, None).unwrap();
        assert!(!loaded.legacy);
        assert_eq!(loaded.source, primary);
        assert_eq!(loaded.auth.sapisid(), "primary");
    }

    #[test]
    fn resolve_falls_back_to_legacy() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path());
        let legacy = write_headers(
            &paths.legacy_config_dir,
            "Cookie: SAPISID=legacy\nUser-Agent: test\n",
        );

        let loaded = resolve_auth(&paths, None).unwrap();
        assert!(loaded.legacy);
        assert_eq!(loaded.source, legacy);
        assert_eq!(loaded.auth.sapisid(), "legacy");
    }

    #[test]
    fn explicit_path_wins() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path());
        write_headers(
            &paths.config_dir,
            "Cookie: SAPISID=primary\nUser-Agent: test\n",
        );
        let explicit = write_headers(
            &dir.path().join("explicit"),
            "Cookie: SAPISID=explicit\nUser-Agent: test\n",
        );

        let loaded = resolve_auth(&paths, Some(&explicit)).unwrap();
        assert_eq!(loaded.source, explicit);
        assert_eq!(loaded.auth.sapisid(), "explicit");
    }

    #[test]
    fn auth_status_maps_errors() {
        let missing = AuthError::NotFound {
            path: PathBuf::from("/config/headers.txt"),
        };
        let status = AuthStatus::from_error(&missing);
        assert_eq!(status.state, AuthState::Missing);
        assert!(status.message.unwrap().contains("not found"));

        let invalid = AuthError::MissingSapisid {
            path: PathBuf::from("/config/headers.txt"),
        };
        let status = AuthStatus::from_error(&invalid);
        assert_eq!(status.state, AuthState::Invalid);
    }
}
