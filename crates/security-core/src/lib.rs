//! Filesystem boundary, path normalization, and secret redaction primitives.

#![forbid(unsafe_code)]

use std::{
    collections::BTreeSet,
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityError {
    RootUnavailable,
    PathEscape,
    SymlinkOutsideRoot,
    InvalidRelativePath,
}

impl SecurityError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::RootUnavailable => "project_path_unavailable",
            Self::PathEscape | Self::InvalidRelativePath => "security_path_escape",
            Self::SymlinkOutsideRoot => "security_symlink_outside_root",
        }
    }
}

impl fmt::Display for SecurityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SecurityError {}

/// A canonical repository root used for every subsequent filesystem check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeRoot {
    canonical: PathBuf,
}

impl SafeRoot {
    /// Canonicalizes an existing directory and uses it as the access boundary.
    ///
    /// # Errors
    ///
    /// Returns `project_path_unavailable` when the path does not exist or is
    /// not a directory.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, SecurityError> {
        let canonical = fs::canonicalize(path).map_err(|_| SecurityError::RootUnavailable)?;
        if !canonical.is_dir() {
            return Err(SecurityError::RootUnavailable);
        }
        Ok(Self { canonical })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.canonical
    }

    /// Resolves an existing path and verifies it remains under the root.
    ///
    /// # Errors
    ///
    /// Returns a path security error when the path cannot be resolved or
    /// escapes the canonical root.
    pub fn resolve_existing(&self, path: impl AsRef<Path>) -> Result<PathBuf, SecurityError> {
        let resolved = fs::canonicalize(path).map_err(|_| SecurityError::PathEscape)?;
        if !is_within(&self.canonical, &resolved) {
            return Err(SecurityError::PathEscape);
        }
        Ok(resolved)
    }

    /// Resolves a path reached through a symlink and returns the dedicated
    /// security error when the link points outside the repository.
    ///
    /// # Errors
    ///
    /// Returns `security_symlink_outside_root` when the resolved target is
    /// outside the repository boundary.
    pub fn resolve_symlink(&self, path: impl AsRef<Path>) -> Result<PathBuf, SecurityError> {
        self.resolve_existing(path).map_err(|error| match error {
            SecurityError::PathEscape => SecurityError::SymlinkOutsideRoot,
            other => other,
        })
    }

    /// Validates a relative path without touching the filesystem.
    ///
    /// # Errors
    ///
    /// Returns `security_path_escape` for absolute paths, parent traversal, or
    /// an empty relative path.
    pub fn relative_path(&self, path: impl AsRef<Path>) -> Result<PathBuf, SecurityError> {
        let relative = normalize_relative_path(path)?;
        let candidate = self.canonical.join(&relative);
        if !is_within(&self.canonical, &candidate) {
            return Err(SecurityError::PathEscape);
        }
        Ok(relative)
    }
}

fn is_within(root: &Path, candidate: &Path) -> bool {
    candidate == root || candidate.starts_with(root)
}

/// Converts a relative path to the stable `/`-separated form stored by the app.
///
/// # Errors
///
/// Returns `security_path_escape` when the path is absolute or contains a
/// parent/root component.
pub fn normalize_relative_path(path: impl AsRef<Path>) -> Result<PathBuf, SecurityError> {
    let path = path.as_ref();
    if path.is_absolute() {
        return Err(SecurityError::InvalidRelativePath);
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SecurityError::InvalidRelativePath);
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(SecurityError::InvalidRelativePath);
    }
    Ok(normalized)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecretFinding {
    pub kind: String,
    pub line: u32,
    pub column: u32,
    pub masked: String,
}

/// Returns true for common private-key and credential filenames.
#[must_use]
pub fn is_sensitive_filename(path: impl AsRef<Path>) -> bool {
    let value = path
        .as_ref()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let extension = std::path::Path::new(&value)
        .extension()
        .and_then(|extension| extension.to_str());
    value == ".env"
        || value.starts_with(".env.")
        || matches!(extension, Some("pem" | "key" | "p12" | "pfx" | "keystore"))
        || value == "id_rsa"
        || value == "id_ed25519"
        || value == "credentials.json"
        || value == "secrets.json"
        || value == ".npmrc"
        || value == ".pypirc"
}

/// Detects common credential patterns while retaining only masked values.
#[must_use]
pub fn detect_secrets(content: &str) -> Vec<SecretFinding> {
    let mut findings = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        let line_number = u32::try_from(line_index + 1).unwrap_or(u32::MAX);
        let lower = line.to_ascii_lowercase();
        let patterns = [
            ("private_key", "-----begin "),
            ("github_token", "ghp_"),
            ("github_token", "github_pat_"),
            ("aws_access_key", "akia"),
            ("bearer_token", "bearer "),
            ("database_url", "postgres://"),
            ("database_url", "mysql://"),
            ("database_url", "mongodb://"),
        ];
        for (kind, pattern) in patterns {
            if let Some(index) = lower.find(pattern) {
                findings.push(SecretFinding {
                    kind: kind.into(),
                    line: line_number,
                    column: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    masked: mask_secret(line.trim()),
                });
            }
        }

        if looks_like_assignment_secret(&lower) {
            findings.push(SecretFinding {
                kind: "assignment_secret".into(),
                line: line_number,
                column: 1,
                masked: mask_secret(line.trim()),
            });
        }
    }
    findings
}

fn looks_like_assignment_secret(line: &str) -> bool {
    let Some((key, value)) = line.split_once('=') else {
        return false;
    };
    let raw_value = value.trim();
    let value = raw_value.trim_matches(['"', '\'', ',', ';', ')', ']']);
    if value.len() < 12 || value.starts_with("${") || is_placeholder_secret(value) {
        return false;
    }

    let key_words = assignment_key_words(key);
    let sensitive_key = key_words
        .iter()
        .any(|word| matches!(word.as_str(), "secret" | "token" | "password"))
        || key_words
            .windows(2)
            .any(|words| matches!(words, [first, second] if (first == "api" || first == "private") && second == "key"));
    if !sensitive_key {
        return false;
    }

    // Expressions in source code are not credentials. Keep accepting compact
    // unquoted env-file values, while ignoring property access and calls such
    // as `config.apiKey`, `process.env.API_KEY`, and `chunk.usage.prompt_tokens`.
    let quoted = raw_value.starts_with(['"', '\'']);
    quoted
        || (!value.chars().any(char::is_whitespace)
            && !value.contains(['.', '(', ')', '?', ':', '+']))
}

fn assignment_key_words(key: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_was_lowercase = false;
    let flush = |current: &mut String, words: &mut Vec<String>| {
        if !current.is_empty() {
            words.push(std::mem::take(current));
        }
    };
    for character in key.chars() {
        if !character.is_ascii_alphanumeric() {
            flush(&mut current, &mut words);
            previous_was_lowercase = false;
            continue;
        }
        if character.is_ascii_uppercase() && previous_was_lowercase {
            flush(&mut current, &mut words);
        }
        current.push(character.to_ascii_lowercase());
        previous_was_lowercase = character.is_ascii_lowercase();
    }
    flush(&mut current, &mut words);
    words
}

fn is_placeholder_secret(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "your-api-key-here"
            | "your-api-key"
            | "changeme"
            | "change-me"
            | "replace-me"
            | "example"
            | "dummy"
    )
}

/// Masks a potentially sensitive value without retaining a usable secret.
#[must_use]
pub fn mask_secret(value: &str) -> String {
    let tail: String = value
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if tail.is_empty() {
        "[REDACTED]".into()
    } else {
        format!("[REDACTED]...{tail}")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafePathMapper {
    used: BTreeSet<String>,
    case_insensitive: bool,
}

impl SafePathMapper {
    #[must_use]
    pub fn new(case_insensitive: bool) -> Self {
        Self {
            used: BTreeSet::new(),
            case_insensitive,
        }
    }

    /// Sanitizes a relative output-like path and avoids reserved names and
    /// case-insensitive collisions deterministically.
    ///
    /// # Errors
    ///
    /// Returns `security_path_escape` when the input is not a safe relative
    /// path.
    pub fn map(&mut self, path: impl AsRef<Path>) -> Result<PathBuf, SecurityError> {
        let normalized = normalize_relative_path(path)?;
        let mut components = Vec::new();
        for component in normalized.components() {
            let value = component.as_os_str().to_string_lossy();
            components.push(sanitize_component(&value));
        }
        let mut mapped = PathBuf::new();
        for component in components {
            mapped.push(component);
        }
        let key = collision_key(&mapped, self.case_insensitive);
        if self.used.insert(key.clone()) {
            return Ok(mapped);
        }

        let digest = blake3::hash(mapped.to_string_lossy().as_bytes());
        let digest_hex = digest.to_hex();
        let suffix = &digest_hex.as_str()[..8];
        let mut collision = mapped.clone();
        let stem = collision
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("file");
        let extension = collision.extension().and_then(|value| value.to_str());
        let replacement = match extension {
            Some(extension) => format!("{stem}-{suffix}.{extension}"),
            None => format!("{stem}-{suffix}"),
        };
        collision.set_file_name(replacement);
        self.used
            .insert(collision_key(&collision, self.case_insensitive));
        Ok(collision)
    }
}

fn collision_key(path: &Path, case_insensitive: bool) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if case_insensitive {
        value.to_ascii_lowercase()
    } else {
        value
    }
}

fn sanitize_component(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect::<String>();
    let stem = sanitized.trim_end_matches([' ', '.']).to_ascii_uppercase();
    if is_windows_reserved_name(&stem) {
        sanitized.insert(0, '_');
    }
    if sanitized.is_empty() {
        sanitized.push('_');
    }
    sanitized
}

#[must_use]
pub fn is_windows_reserved_name(value: &str) -> bool {
    let stem = value
        .rsplit_once('.')
        .map_or(value, |(stem, _)| stem)
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
}

#[cfg(test)]
mod tests {
    use super::{
        detect_secrets, is_sensitive_filename, is_windows_reserved_name, mask_secret,
        SafePathMapper, SafeRoot,
    };

    #[test]
    fn secret_detection_never_returns_the_original_value() {
        let findings = detect_secrets("OPENAI_API_KEY=super-secret-value");
        assert_eq!(findings.len(), 1);
        assert!(!findings[0].masked.contains("super-secret-value"));
        assert_eq!(findings[0].kind, "assignment_secret");
    }

    #[test]
    fn assignment_detection_ignores_source_expressions_and_placeholders() {
        let source = [
            "config.llm.apiKey = key.slice(0, 8) + \"...\" + key.slice(-4);",
            "promptTokens = chunk.usage.prompt_tokens ?? 0;",
            "const apiKey = process.env.INKOS_LLM_API_KEY ?? \"\";",
            "\"INKOS_LLM_API_KEY=your-api-key-here\",",
        ]
        .join("\n");
        assert!(detect_secrets(&source).is_empty());
    }

    #[test]
    fn sensitive_filenames_and_reserved_names_are_detected() {
        assert!(is_sensitive_filename("config/.env.production"));
        assert!(is_windows_reserved_name("CON.txt"));
        assert!(is_windows_reserved_name("LPT9"));
        assert!(!is_windows_reserved_name("COM0"));
    }

    #[test]
    fn path_mapper_resolves_reserved_names_and_case_collisions() {
        let mut mapper = SafePathMapper::new(true);
        assert_eq!(
            mapper.map("CON.txt").unwrap(),
            std::path::PathBuf::from("_CON.txt")
        );
        assert_eq!(
            mapper.map("src/Thing.rs").unwrap(),
            std::path::PathBuf::from("src/Thing.rs")
        );
        assert!(mapper
            .map("src/thing.rs")
            .unwrap()
            .to_string_lossy()
            .contains('-'));
    }

    #[test]
    fn masking_keeps_only_a_short_non_secret_tail() {
        assert_eq!(mask_secret("secret-value"), "[REDACTED]...alue");
    }

    #[test]
    fn root_rejects_relative_escape() {
        let directory = tempfile_directory();
        let root = SafeRoot::new(&directory).unwrap();
        assert!(root.relative_path("../outside").is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn tempfile_directory() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("security-core-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
