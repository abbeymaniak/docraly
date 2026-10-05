pub mod php;

use std::path::Path;

use crate::discovery::Language;
use crate::model::{FileModel, ProjectModel};
use crate::scanner::SourceFile;

/// Common interface that all language adapters implement to decouple parsing from the CLI.
pub trait LanguageAdapter: Send + Sync {
    /// Language supported by this adapter.
    fn language(&self) -> Language;

    /// File extensions handled by this adapter (e.g. `["php"]`).
    #[allow(dead_code)]
    fn extensions(&self) -> &'static [&'static str];

    /// Parse a single source file into Docraly's internal FileModel.
    fn parse_file(
        &self,
        path: &Path,
        relative_path: &Path,
        source: &str,
    ) -> Result<FileModel, String>;
}

/// Registry of available language adapters.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn LanguageAdapter>>,
}

impl AdapterRegistry {
    /// Creates a registry initialized with default language adapters.
    pub fn new() -> Self {
        let mut registry = Self {
            adapters: Vec::new(),
        };
        registry.register(Box::new(php::PhpAdapter::new()));
        registry
    }

    /// Registers a new language adapter.
    pub fn register(&mut self, adapter: Box<dyn LanguageAdapter>) {
        self.adapters.push(adapter);
    }

    /// Retrieves an adapter by Language.
    pub fn get(&self, language: Language) -> Option<&dyn LanguageAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.language() == language)
            .map(|adapter| adapter.as_ref())
    }

    /// Parses a collection of discovered source files into a unified ProjectModel.
    pub fn parse_project(&self, source_files: &[SourceFile]) -> ProjectModel {
        let mut files = Vec::new();

        for file in source_files {
            if let Some(adapter) = self.get(file.language) {
                if let Ok(source) = std::fs::read_to_string(&file.path) {
                    if let Ok(file_model) =
                        adapter.parse_file(&file.path, &file.relative_path, &source)
                    {
                        files.push(file_model);
                    }
                }
            }
        }

        ProjectModel { files }
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "docraly_test_adapter_{}_{}",
                name,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }

        fn write_file(&self, relative: &str, content: &str) -> PathBuf {
            let full = self.path.join(relative);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&full, content).unwrap();
            full
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_adapter_registry_parse_multiple_files() {
        let temp = TempDir::new("multi_file");
        let user_controller_path = temp.write_file(
            "app/Http/Controllers/UserController.php",
            r#"<?php
namespace App\Http\Controllers;

use App\Models\User;

class UserController {
    public function show(?int $id): ?User {
        return null;
    }
}
"#,
        );

        let post_controller_path = temp.write_file(
            "app/Http/Controllers/PostController.php",
            r#"<?php
namespace App\Http\Controllers;

class PostController {
    public function index(): array {
        return [];
    }
}
"#,
        );

        let source_files = vec![
            SourceFile {
                path: user_controller_path,
                relative_path: PathBuf::from("app/Http/Controllers/UserController.php"),
                language: Language::Php,
            },
            SourceFile {
                path: post_controller_path,
                relative_path: PathBuf::from("app/Http/Controllers/PostController.php"),
                language: Language::Php,
            },
        ];

        let registry = AdapterRegistry::new();
        let project_model = registry.parse_project(&source_files);

        assert_eq!(project_model.files.len(), 2);

        let user_class = project_model
            .find_class_by_fqcn("App\\Http\\Controllers\\UserController")
            .unwrap();
        assert_eq!(user_class.name, "UserController");
        assert_eq!(user_class.methods.len(), 1);
        assert_eq!(user_class.methods[0].name, "show");

        let post_class = project_model
            .find_class_by_fqcn("App\\Http\\Controllers\\PostController")
            .unwrap();
        assert_eq!(post_class.name, "PostController");
        assert_eq!(post_class.methods.len(), 1);
        assert_eq!(post_class.methods[0].name, "index");
    }
}
