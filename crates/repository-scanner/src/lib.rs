//! Cancellable repository scanning with nested Git Ignore and security filters.

#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::SystemTime,
};

use batch_code_analyzer_security_core::{
    detect_secrets, is_sensitive_filename, SafeRoot, SecretFinding,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_FILE_SIZE: u64 = 256 * 1024;
pub const DEFAULT_MAX_FILES: usize = 10_000;

#[derive(Clone, Debug)]
pub struct ScanCancellation(Arc<AtomicBool>);

impl Default for ScanCancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub struct ScanConfig {
    pub root: PathBuf,
    pub max_file_size: u64,
    pub max_files: usize,
    pub include_extensions: BTreeSet<String>,
    pub excluded_directories: BTreeSet<String>,
    pub excluded_extensions: BTreeSet<String>,
    pub excluded_patterns: Vec<String>,
    pub use_gitignore: bool,
    pub detect_sensitive_content: bool,
    pub cancellation: ScanCancellation,
}

impl ScanConfig {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_files: DEFAULT_MAX_FILES,
            include_extensions: BTreeSet::new(),
            excluded_directories: default_excluded_directories(),
            excluded_extensions: default_excluded_extensions(),
            excluded_patterns: Vec::new(),
            use_gitignore: true,
            detect_sensitive_content: true,
            cancellation: ScanCancellation::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FileDecision {
    Included,
    Excluded { reason: String },
    Unreadable,
    Binary,
    UnsupportedEncoding,
    Sensitive,
    TooLarge,
    Symlink,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScannedFile {
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<String>,
    pub content_hash: Option<String>,
    pub encoding: Option<String>,
    pub language: Option<String>,
    pub decision: FileDecision,
    pub sensitive_findings: Vec<SecretFinding>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    pub visited_entries: u64,
    pub scanned_files: u64,
    pub included_files: u64,
    pub excluded_by_reason: BTreeMap<String, u64>,
    pub unreadable_files: Vec<String>,
    pub unsupported_encoding_files: Vec<String>,
    pub sensitive_files: Vec<String>,
    pub symlink_files: Vec<String>,
    pub invalid_gitignore_rules: Vec<String>,
    pub builtin_directories: Vec<String>,
    pub builtin_extensions: Vec<String>,
    pub gitignore_rules: Vec<String>,
    pub temporary_excluded_patterns: Vec<String>,
    pub sensitive_detection_enabled: bool,
    pub cancelled: bool,
}

impl ImportReport {
    fn count_exclusion(&mut self, reason: &str) {
        *self.excluded_by_reason.entry(reason.into()).or_default() += 1;
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScanResult {
    pub files: Vec<ScannedFile>,
    pub report: ImportReport,
    pub completed: bool,
}

#[derive(Clone, Debug)]
pub struct Scanner {
    config: ScanConfig,
}

impl Scanner {
    #[must_use]
    pub fn new(config: ScanConfig) -> Self {
        Self { config }
    }

    /// Scans a repository synchronously. Callers should run it off the UI thread.
    ///
    /// Cancellation returns `completed = false`; partial files are diagnostic
    /// only and must not be committed as a formal scan generation.
    ///
    /// # Errors
    ///
    /// Returns a stable scan error when the root cannot be opened or a
    /// directory entry cannot be read.
    pub fn scan(&self) -> Result<ScanResult, ScanError> {
        let root =
            SafeRoot::new(&self.config.root).map_err(|error| ScanError::Root(error.code()))?;
        let mut report = ImportReport {
            builtin_directories: self.config.excluded_directories.iter().cloned().collect(),
            builtin_extensions: self.config.excluded_extensions.iter().cloned().collect(),
            temporary_excluded_patterns: self.config.excluded_patterns.clone(),
            sensitive_detection_enabled: self.config.detect_sensitive_content,
            ..ImportReport::default()
        };
        let mut files = Vec::new();
        let mut rules = Vec::new();
        if self.config.use_gitignore {
            load_gitignore(&root, Path::new(""), &mut rules, &mut report)?;
        }
        walk_directory(
            &root,
            Path::new(""),
            &self.config,
            &rules,
            &mut files,
            &mut report,
        )?;
        let completed = !self.config.cancellation.is_cancelled();
        report.cancelled = !completed;
        Ok(ScanResult {
            files,
            report,
            completed,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScanError {
    Root(&'static str),
    Io(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root(code) => formatter.write_str(code),
            Self::Io(path) => write!(formatter, "scan_file_unreadable:{path}"),
        }
    }
}

impl std::error::Error for ScanError {}

#[derive(Clone, Debug)]
struct IgnoreRule {
    pattern: String,
    negated: bool,
    directory_only: bool,
    base: PathBuf,
}

fn walk_directory(
    root: &SafeRoot,
    relative_directory: &Path,
    config: &ScanConfig,
    inherited_rules: &[IgnoreRule],
    files: &mut Vec<ScannedFile>,
    report: &mut ImportReport,
) -> Result<(), ScanError> {
    if config.cancellation.is_cancelled() {
        return Ok(());
    }
    let absolute_directory = root.path().join(relative_directory);
    let mut rules = inherited_rules.to_vec();
    if config.use_gitignore {
        load_gitignore(root, relative_directory, &mut rules, report)?;
    }
    let entries = fs::read_dir(&absolute_directory)
        .map_err(|_| ScanError::Io(relative_directory.display().to_string()))?;
    for entry in entries {
        if config.cancellation.is_cancelled() || files.len() >= config.max_files {
            break;
        }
        let entry = entry.map_err(|_| ScanError::Io(relative_directory.display().to_string()))?;
        report.visited_entries += 1;
        let path = entry.path();
        let relative = relative_directory.join(entry.file_name());
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| ScanError::Io(relative.display().to_string()))?;
        if metadata.file_type().is_symlink() {
            report.symlink_files.push(normalize_display_path(&relative));
            report.count_exclusion("symlink");
            files.push(ScannedFile {
                relative_path: normalize_display_path(&relative),
                size_bytes: 0,
                modified_at: None,
                content_hash: None,
                encoding: None,
                language: None,
                decision: FileDecision::Symlink,
                sensitive_findings: Vec::new(),
            });
            continue;
        }
        if metadata.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if config.excluded_directories.contains(&name) {
                report.count_exclusion("builtin_directory");
                continue;
            }
            if ignored(&relative, true, &rules) || matches_user_pattern(&relative, true, config) {
                report.count_exclusion("gitignore_or_user_pattern");
            }
            walk_directory(root, &relative, config, &rules, files, report)?;
            continue;
        }
        report.scanned_files += 1;
        process_file(root, &relative, &metadata, config, &rules, files, report);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn process_file(
    root: &SafeRoot,
    relative: &Path,
    metadata: &fs::Metadata,
    config: &ScanConfig,
    rules: &[IgnoreRule],
    files: &mut Vec<ScannedFile>,
    report: &mut ImportReport,
) {
    let relative_path = normalize_display_path(relative);
    let excluded_extension = relative
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            config
                .excluded_extensions
                .contains(&value.to_ascii_lowercase())
        });
    let ignored = ignored(relative, false, rules) || matches_user_pattern(relative, false, config);
    let decision = if is_sensitive_filename(relative) {
        Some("sensitive_filename")
    } else if ignored {
        Some("gitignore_or_user_pattern")
    } else if excluded_extension {
        Some("builtin_extension")
    } else if metadata.len() > config.max_file_size {
        Some("file_too_large")
    } else if !config.include_extensions.is_empty()
        && !relative
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                config
                    .include_extensions
                    .contains(&value.to_ascii_lowercase())
            })
    {
        Some("not_included_extension")
    } else {
        None
    };
    if let Some(reason) = decision {
        report.count_exclusion(reason);
        if reason == "sensitive_filename" {
            report.sensitive_files.push(relative_path.clone());
        }
        let file_decision = match reason {
            "sensitive_filename" => FileDecision::Sensitive,
            "file_too_large" => FileDecision::TooLarge,
            _ => FileDecision::Excluded {
                reason: reason.into(),
            },
        };
        files.push(ScannedFile {
            relative_path,
            size_bytes: metadata.len(),
            modified_at: modified_at(metadata),
            content_hash: None,
            encoding: None,
            language: None,
            decision: file_decision,
            sensitive_findings: Vec::new(),
        });
        return;
    }

    let absolute = root.path().join(relative);
    let Ok(bytes) = fs::read(&absolute) else {
        report.unreadable_files.push(relative_path.clone());
        report.count_exclusion("unreadable");
        files.push(ScannedFile {
            relative_path,
            size_bytes: metadata.len(),
            modified_at: modified_at(metadata),
            content_hash: None,
            encoding: None,
            language: None,
            decision: FileDecision::Unreadable,
            sensitive_findings: Vec::new(),
        });
        return;
    };
    if bytes.contains(&0) {
        report.count_exclusion("binary");
        files.push(file_with_decision(
            relative_path,
            metadata.len(),
            metadata,
            FileDecision::Binary,
        ));
        return;
    }
    let Ok(content) =
        std::str::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
    else {
        report
            .unsupported_encoding_files
            .push(relative_path.clone());
        report.count_exclusion("unsupported_encoding");
        files.push(file_with_decision(
            relative_path,
            metadata.len(),
            metadata,
            FileDecision::UnsupportedEncoding,
        ));
        return;
    };
    let findings = if config.detect_sensitive_content {
        detect_secrets(content)
    } else {
        Vec::new()
    };
    if !findings.is_empty() {
        report.sensitive_files.push(relative_path.clone());
        report.count_exclusion("sensitive_content");
        files.push(ScannedFile {
            relative_path,
            size_bytes: metadata.len(),
            modified_at: modified_at(metadata),
            content_hash: None,
            encoding: Some("utf-8".into()),
            language: language_for(relative),
            decision: FileDecision::Sensitive,
            sensitive_findings: findings,
        });
        return;
    }
    report.included_files += 1;
    files.push(ScannedFile {
        relative_path,
        size_bytes: metadata.len(),
        modified_at: modified_at(metadata),
        content_hash: Some(format!("blake3:{}", blake3::hash(&bytes).to_hex())),
        encoding: Some("utf-8".into()),
        language: language_for(relative),
        decision: FileDecision::Included,
        sensitive_findings: Vec::new(),
    });
}

fn file_with_decision(
    path: String,
    size: u64,
    metadata: &fs::Metadata,
    decision: FileDecision,
) -> ScannedFile {
    ScannedFile {
        relative_path: path,
        size_bytes: size,
        modified_at: modified_at(metadata),
        content_hash: None,
        encoding: None,
        language: None,
        decision,
        sensitive_findings: Vec::new(),
    }
}

fn modified_at(metadata: &fs::Metadata) -> Option<String> {
    metadata.modified().ok().and_then(|time| {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|duration| format!("unix:{}", duration.as_secs()))
    })
}

fn language_for(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let language = match extension.as_str() {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "md" => "markdown",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        _ => return None,
    };
    Some(language.into())
}

fn load_gitignore(
    root: &SafeRoot,
    base: &Path,
    rules: &mut Vec<IgnoreRule>,
    report: &mut ImportReport,
) -> Result<(), ScanError> {
    let path = root.path().join(base).join(".gitignore");
    if !path.is_file() {
        return Ok(());
    }
    let content =
        fs::read_to_string(path).map_err(|_| ScanError::Io(base.display().to_string()))?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match parse_rule(trimmed, base) {
            Ok(rule) => {
                if !report.gitignore_rules.iter().any(|value| value == trimmed) {
                    report.gitignore_rules.push(trimmed.into());
                }
                rules.push(rule);
            }
            Err(()) => report.invalid_gitignore_rules.push(trimmed.into()),
        }
    }
    Ok(())
}

fn parse_rule(value: &str, base: &Path) -> Result<IgnoreRule, ()> {
    let mut pattern = value.trim().to_owned();
    let negated = pattern.starts_with('!');
    if negated {
        pattern.remove(0);
    }
    if pattern.is_empty() || pattern.contains('\0') {
        return Err(());
    }
    let directory_only = pattern.ends_with('/');
    if directory_only {
        pattern.pop();
    }
    if pattern.starts_with('/') {
        pattern.remove(0);
    }
    Ok(IgnoreRule {
        pattern,
        negated,
        directory_only,
        base: base.to_path_buf(),
    })
}

fn ignored(path: &Path, is_directory: bool, rules: &[IgnoreRule]) -> bool {
    let mut ignored = false;
    for rule in rules {
        let relative = path.strip_prefix(&rule.base).unwrap_or(path);
        let matches = if rule.directory_only {
            if is_directory {
                rule_matches_path(&rule.pattern, relative)
            } else {
                relative
                    .ancestors()
                    .skip(1)
                    .any(|ancestor| rule_matches_path(&rule.pattern, ancestor))
            }
        } else {
            rule_matches_path(&rule.pattern, relative)
        };
        if matches {
            ignored = !rule.negated;
        }
    }
    ignored
}

fn matches_user_pattern(path: &Path, is_directory: bool, config: &ScanConfig) -> bool {
    config.excluded_patterns.iter().any(|pattern| {
        parse_rule(pattern, Path::new("")).is_ok_and(|rule| {
            if rule.directory_only && !is_directory {
                path.ancestors()
                    .skip(1)
                    .any(|ancestor| rule_matches_path(&rule.pattern, ancestor))
            } else {
                rule_matches_path(&rule.pattern, path)
            }
        })
    })
}

fn rule_matches_path(pattern: &str, path: &Path) -> bool {
    let value = normalize_display_path(path);
    if pattern.contains('/') {
        glob_match(pattern, &value)
    } else {
        value
            .split('/')
            .any(|component| glob_match(pattern, component))
    }
}

fn glob_match(pattern: &str, value: &str) -> bool {
    glob_match_bytes(pattern.as_bytes(), value.as_bytes())
}

fn glob_match_bytes(pattern: &[u8], value: &[u8]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    if pattern[0] == b'*' {
        if pattern.get(1) == Some(&b'*') {
            return glob_match_bytes(&pattern[1..], value)
                || (!value.is_empty() && glob_match_bytes(pattern, &value[1..]));
        }
        return glob_match_bytes(&pattern[1..], value)
            || (!value.is_empty() && value[0] != b'/' && glob_match_bytes(pattern, &value[1..]));
    }
    if pattern[0] == b'?' {
        return !value.is_empty()
            && value[0] != b'/'
            && glob_match_bytes(&pattern[1..], &value[1..]);
    }
    !value.is_empty() && pattern[0] == value[0] && glob_match_bytes(&pattern[1..], &value[1..])
}

fn normalize_display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn default_excluded_directories() -> BTreeSet<String> {
    [
        ".git",
        ".svn",
        ".hg",
        "node_modules",
        "vendor",
        "dist",
        "build",
        "out",
        "target",
        ".next",
        ".nuxt",
        ".cache",
        "coverage",
        "__pycache__",
        ".pytest_cache",
        ".idea",
        ".vscode",
        ".batch-analysis",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn default_excluded_extensions() -> BTreeSet<String> {
    [
        "zip", "tar", "gz", "rar", "7z", "exe", "dll", "so", "dylib", "bin", "png", "jpg", "jpeg",
        "gif", "webp", "ico", "mp3", "mp4", "mov", "pdf", "woff", "woff2", "ttf", "otf", "lock",
        "db", "sqlite", "sqlite3",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{FileDecision, ScanCancellation, ScanConfig, Scanner};

    #[test]
    fn nested_gitignore_supports_negation_and_reports_included_files() {
        let root = temp_root("gitignore");
        fs::write(root.join(".gitignore"), "ignored/\n!ignored/keep.rs\n").unwrap();
        fs::create_dir_all(root.join("ignored")).unwrap();
        fs::write(root.join("ignored/drop.rs"), "fn drop() {}\n").unwrap();
        fs::write(root.join("ignored/keep.rs"), "fn keep() {}\n").unwrap();

        let result = Scanner::new(ScanConfig::new(&root)).scan().unwrap();
        assert!(result.files.iter().any(|file| {
            file.relative_path == "ignored/keep.rs" && file.decision == FileDecision::Included
        }));
        assert!(result
            .report
            .excluded_by_reason
            .contains_key("gitignore_or_user_pattern"));
        assert_eq!(
            result.report.gitignore_rules,
            vec!["ignored/".to_owned(), "!ignored/keep.rs".to_owned()]
        );
        assert!(result
            .report
            .builtin_directories
            .contains(&"node_modules".to_owned()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn temporary_patterns_are_reported_and_exclude_matching_files() {
        let root = temp_root("temporary-patterns");
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/draft.rs"), "fn draft() {}\n").unwrap();
        let mut config = ScanConfig::new(&root);
        config.excluded_patterns = vec!["notes/**".into()];

        let result = Scanner::new(config).scan().unwrap();
        assert_eq!(
            result.report.temporary_excluded_patterns,
            vec!["notes/**".to_owned()]
        );
        assert!(result.files.iter().any(|file| {
            file.relative_path == "notes/draft.rs"
                && file.decision
                    == FileDecision::Excluded {
                        reason: "gitignore_or_user_pattern".into(),
                    }
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scanner_reports_binary_large_and_sensitive_files_without_hashing_them() {
        let root = temp_root("filters");
        fs::write(root.join("binary.bin"), [0, 1, 2]).unwrap();
        fs::write(root.join(".env"), "API_KEY=super-secret-value\n").unwrap();
        fs::write(root.join(".gitignore"), ".env\n").unwrap();
        fs::write(root.join("large.rs"), vec![b'a'; 20]).unwrap();
        let mut config = ScanConfig::new(&root);
        config.max_file_size = 10;
        config.excluded_extensions.clear();
        let result = Scanner::new(config).scan().unwrap();
        assert!(result
            .files
            .iter()
            .any(|file| file.decision == FileDecision::Binary));
        assert!(result
            .files
            .iter()
            .any(|file| file.decision == FileDecision::Sensitive));
        assert!(result
            .files
            .iter()
            .any(|file| file.decision == FileDecision::TooLarge));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn outside_symlink_is_reported_without_reading_its_target() {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink");
        let outside = temp_root("outside");
        fs::write(outside.join("secret.rs"), "API_KEY=outside-secret-value\n").unwrap();
        symlink(outside.join("secret.rs"), root.join("linked.rs")).unwrap();

        let result = Scanner::new(ScanConfig::new(&root)).scan().unwrap();
        let linked = result
            .files
            .iter()
            .find(|file| file.relative_path == "linked.rs")
            .expect("symlink should be reported");
        assert_eq!(linked.decision, FileDecision::Symlink);
        assert!(!result
            .report
            .sensitive_files
            .iter()
            .any(|path| path == "linked.rs"));
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn cancellation_marks_result_incomplete_without_formal_commit_signal() {
        let root = temp_root("cancel");
        fs::write(root.join("a.rs"), "fn main() {}\n").unwrap();
        let cancellation = ScanCancellation::new();
        cancellation.cancel();
        let mut config = ScanConfig::new(&root);
        config.cancellation = cancellation;
        let result = Scanner::new(config).scan().unwrap();
        assert!(!result.completed);
        assert!(result.report.cancelled);
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("repository-scanner-{label}-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
