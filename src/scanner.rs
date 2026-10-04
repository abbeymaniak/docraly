use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{PathBuf};
use walkdir::{DirEntry, WalkDir};

use crate::discovery::{Framework, Language, ProjectInfo};

/// Directories that should be skipped during source file traversal.
pub const DEFAULT_IGNORED_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "vendor",
    "node_modules",
    "storage",
    "target",
    "__pycache__",
    ".idea",
    ".vscode",
    ".pytest_cache",
    ".mypy_cache",
    "dist",
    "build",
];

/// Represents a source file discovered in the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// Path relative to the project root.
    pub relative_path: PathBuf,
    /// The language identified for this file.
    pub language: Language,
}

/// Map a file extension to its corresponding Language enum.
pub fn language_for_extension(ext: &str) -> Option<Language> {
    match ext.to_lowercase().as_str() {
        "php" => Some(Language::Php),
        "py" => Some(Language::Python),
        "rs" => Some(Language::Rust),
        "js" | "jsx" | "mjs" | "cjs" => Some(Language::JavaScript),
        "ts" | "tsx" | "mts" | "cts" => Some(Language::TypeScript),
        "go" => Some(Language::Go),
        _ => None,
    }
}

/// Returns the file extensions associated with a given language.
#[allow(dead_code)]
pub fn extensions_for_language(language: Language) -> &'static [&'static str] {
    match language {
        Language::Php => &["php"],
        Language::Python => &["py"],
        Language::Rust => &["rs"],
        Language::JavaScript => &["js", "jsx", "mjs", "cjs"],
        Language::TypeScript => &["ts", "tsx", "mts", "cts"],
        Language::Go => &["go"],
    }
}

/// Checks if a directory entry should be pruned from traversal.
fn is_ignored_entry(entry: &DirEntry) -> bool {
    if entry.file_type().is_dir() {
        if let Some(name) = entry.file_name().to_str() {
            return DEFAULT_IGNORED_DIRS.contains(&name);
        }
    }
    false
}

/// Determine target directories to scan based on detected frameworks and languages.
pub fn target_directories(project: &ProjectInfo) -> Vec<PathBuf> {
    let mut targets = Vec::new();

    // 1. Framework-specific targeting
    for framework in &project.frameworks {
        match framework {
            Framework::Laravel => {
                for dir_name in &["app", "routes"] {
                    let dir = project.root.join(dir_name);
                    if dir.is_dir() && !targets.contains(&dir) {
                        targets.push(dir);
                    }
                }
            }
            Framework::Symfony => {
                for dir_name in &["src", "config"] {
                    let dir = project.root.join(dir_name);
                    if dir.is_dir() && !targets.contains(&dir) {
                        targets.push(dir);
                    }
                }
            }
            Framework::FastApi | Framework::Django => {
                for dir_name in &["app", "src"] {
                    let dir = project.root.join(dir_name);
                    if dir.is_dir() && !targets.contains(&dir) {
                        targets.push(dir);
                    }
                }
            }
            Framework::Axum | Framework::Actix => {
                let dir = project.root.join("src");
                if dir.is_dir() && !targets.contains(&dir) {
                    targets.push(dir);
                }
            }
            Framework::Express | Framework::NestJs => {
                for dir_name in &["src", "app", "routes"] {
                    let dir = project.root.join(dir_name);
                    if dir.is_dir() && !targets.contains(&dir) {
                        targets.push(dir);
                    }
                }
            }
        }
    }

    // 2. Language-specific directories if no framework matched
    if targets.is_empty() {
        for language in &project.languages {
            match language {
                Language::Rust => {
                    let dir = project.root.join("src");
                    if dir.is_dir() && !targets.contains(&dir) {
                        targets.push(dir);
                    }
                }
                Language::TypeScript | Language::JavaScript => {
                    let dir = project.root.join("src");
                    if dir.is_dir() && !targets.contains(&dir) {
                        targets.push(dir);
                    }
                }
                Language::Php => {
                    for dir_name in &["src", "app"] {
                        let dir = project.root.join(dir_name);
                        if dir.is_dir() && !targets.contains(&dir) {
                            targets.push(dir);
                        }
                    }
                }
                Language::Python | Language::Go => {}
            }
        }
    }

    // 3. Generic fallback: if no specific subdirectories found, scan root
    if targets.is_empty() {
        targets.push(project.root.clone());
    }

    targets
}

/// Scans the project for relevant source files matching the project profile.
pub fn scan_project_sources(project: &ProjectInfo) -> Vec<SourceFile> {
    let targets = target_directories(project);
    let mut files = Vec::new();
    let mut visited = HashSet::new();

    // If project.languages is empty, allow all supported languages;
    // otherwise, filter to the project's detected languages.
    let filter_by_languages = !project.languages.is_empty();

    for target in targets {
        if !target.exists() {
            continue;
        }

        for entry in WalkDir::new(&target)
            .into_iter()
            .filter_entry(|entry| !is_ignored_entry(entry))
            .filter_map(|entry| entry.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path().to_path_buf();

            // Deduplicate across targets
            if !visited.insert(path.clone()) {
                continue;
            }

            let Some(ext) = path.extension().and_then(OsStr::to_str) else {
                continue;
            };

            let Some(language) = language_for_extension(ext) else {
                continue;
            };

            if filter_by_languages && !project.languages.contains(&language) {
                continue;
            }

            let relative_path = path
                .strip_prefix(&project.root)
                .unwrap_or(&path)
                .to_path_buf();

            files.push(SourceFile {
                path,
                relative_path,
                language,
            });
        }
    }

    // Sort by relative path for deterministic ordering
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "docraly_test_{}_{}",
                name,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }

        fn create_file(&self, relative: &str) {
            let full = self.path.join(relative);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            File::create(full).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_language_for_extension() {
        assert_eq!(language_for_extension("php"), Some(Language::Php));
        assert_eq!(language_for_extension("py"), Some(Language::Python));
        assert_eq!(language_for_extension("rs"), Some(Language::Rust));
        assert_eq!(language_for_extension("ts"), Some(Language::TypeScript));
        assert_eq!(language_for_extension("js"), Some(Language::JavaScript));
        assert_eq!(language_for_extension("go"), Some(Language::Go));
        assert_eq!(language_for_extension("txt"), None);

        assert_eq!(extensions_for_language(Language::Php), &["php"]);
        assert_eq!(extensions_for_language(Language::Rust), &["rs"]);
        assert_eq!(extensions_for_language(Language::Python), &["py"]);
    }

    #[test]
    fn test_scan_ignores_vendor_and_git() {
        let temp = TempDir::new("ignore_test");
        temp.create_file("src/main.rs");
        temp.create_file(".git/config");
        temp.create_file("vendor/autoload.php");
        temp.create_file("node_modules/pkg/index.js");
        temp.create_file("target/debug/build.rs");
        temp.create_file("storage/logs/laravel.log");

        let project = ProjectInfo {
            root: temp.path.clone(),
            languages: vec![Language::Rust, Language::Php, Language::JavaScript],
            frameworks: vec![],
        };

        let files = scan_project_sources(&project);
        let rel_paths: Vec<_> = files
            .iter()
            .map(|f| f.relative_path.to_str().unwrap())
            .collect();

        assert_eq!(rel_paths, vec!["src/main.rs"]);
    }

    #[test]
    fn test_scan_laravel_strategy() {
        let temp = TempDir::new("laravel_test");
        temp.create_file("app/Http/Controllers/UserController.php");
        temp.create_file("app/Models/User.php");
        temp.create_file("routes/api.php");
        temp.create_file("routes/web.php");
        temp.create_file("vendor/laravel/framework/src/Illuminate/Foundation/Application.php");
        temp.create_file("storage/framework/views/cache.php");
        temp.create_file("database/migrations/0001_create_users_table.php");

        let project = ProjectInfo {
            root: temp.path.clone(),
            languages: vec![Language::Php],
            frameworks: vec![Framework::Laravel],
        };

        let files = scan_project_sources(&project);
        let rel_paths: Vec<_> = files
            .iter()
            .map(|f| f.relative_path.to_str().unwrap())
            .collect();

        assert_eq!(
            rel_paths,
            vec![
                "app/Http/Controllers/UserController.php",
                "app/Models/User.php",
                "routes/api.php",
                "routes/web.php",
            ]
        );
    }

    #[test]
    fn test_scan_generic_fallback() {
        let temp = TempDir::new("generic_test");
        temp.create_file("lib/utils.py");
        temp.create_file("server.py");
        temp.create_file("__pycache__/server.cpython-312.pyc");

        let project = ProjectInfo {
            root: temp.path.clone(),
            languages: vec![Language::Python],
            frameworks: vec![],
        };

        let files = scan_project_sources(&project);
        let rel_paths: Vec<_> = files
            .iter()
            .map(|f| f.relative_path.to_str().unwrap())
            .collect();

        assert_eq!(rel_paths, vec!["lib/utils.py", "server.py"]);
    }
}
