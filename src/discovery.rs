use std::path::{Path, PathBuf};

use crate::manifest::{read_json_file, read_toml_file};

#[derive(Debug)]
pub struct ProjectInfo {
    pub root: PathBuf,
    pub languages: Vec<Language>,
    pub frameworks: Vec<Framework>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Php,
    Python,
    Rust,
    JavaScript,
    TypeScript,
    Go,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Framework {
    Laravel,
    Symfony,
    FastApi,
    Django,
    Axum,
    Actix,
    Express,
    NestJs,
}


fn add_language(languages: &mut Vec<Language>, language: Language) {
    if !languages.contains(&language) {
        languages.push(language);
    }
}



fn add_framework(frameworks: &mut Vec<Framework>, framework: Framework) {
    if !frameworks.contains(&framework) {
        frameworks.push(framework);
    }
}

pub fn discover_project(root: &Path) -> ProjectInfo {
    let mut languages = Vec::new();
    let mut frameworks = Vec::new();

    detect_php(root, &mut languages, &mut frameworks);
    detect_python(root, &mut languages, &mut frameworks);
    detect_rust(root, &mut languages, &mut frameworks);
    detect_javascript(root, &mut languages, &mut frameworks);
    detect_go(root, &mut languages, &mut frameworks);

    ProjectInfo {
        root: root.to_path_buf(),
        languages,
        frameworks,
    }
}   

// Detect PHP
fn detect_php(
    root: &Path,
    languages: &mut Vec<Language>,
    frameworks: &mut Vec<Framework>,
) {
    let path = root.join("composer.json");

    if !path.exists() {
        return;
    }

    add_language(languages, Language::Php);

    let Ok(manifest) = read_json_file(&path) else {
        return;
    };

    for group in ["require", "require-dev"] {
        let Some(packages) = manifest
            .get(group)
            .and_then(|value| value.as_object())
        else {
            continue;
        };

        for package in packages.keys() {
            match package.as_str() {
                "laravel/framework" => {
                    add_framework(frameworks, Framework::Laravel);
                }

                "symfony/framework-bundle" | "symfony/symfony" => {
                    add_framework(frameworks, Framework::Symfony);
                }

                _ => {}
            }
        }
    }
}

// Detect Python
fn detect_python(
    root: &Path,
    languages: &mut Vec<Language>,
    frameworks: &mut Vec<Framework>,
) {
    let mut is_python = root.join("manage.py").exists();
    let mut packages: Vec<String> = Vec::new();

    // requirements.txt: one requirement per line
    if let Ok(contents) = std::fs::read_to_string(root.join("requirements.txt")) {
        is_python = true;
        packages.extend(contents.lines().filter_map(python_package_name));
    }

    // pyproject.toml: PEP 621 (`[project]`) and Poetry (`[tool.poetry]`)
    let pyproject = root.join("pyproject.toml");

    if pyproject.exists() {
        is_python = true;

        if let Ok(manifest) = read_toml_file(&pyproject) {
            if let Some(dependencies) = manifest
                .get("project")
                .and_then(|project| project.get("dependencies"))
                .and_then(|dependencies| dependencies.as_array())
            {
                packages.extend(
                    dependencies
                        .iter()
                        .filter_map(|dependency| dependency.as_str())
                        .filter_map(python_package_name),
                );
            }

            if let Some(dependencies) = manifest
                .get("tool")
                .and_then(|tool| tool.get("poetry"))
                .and_then(|poetry| poetry.get("dependencies"))
                .and_then(|dependencies| dependencies.as_table())
            {
                packages.extend(dependencies.keys().map(|name| name.to_lowercase()));
            }
        }
    }

    // Pipfile: TOML with `[packages]` and `[dev-packages]`
    let pipfile = root.join("Pipfile");

    if pipfile.exists() {
        is_python = true;

        if let Ok(manifest) = read_toml_file(&pipfile) {
            for group in ["packages", "dev-packages"] {
                if let Some(dependencies) = manifest
                    .get(group)
                    .and_then(|dependencies| dependencies.as_table())
                {
                    packages.extend(dependencies.keys().map(|name| name.to_lowercase()));
                }
            }
        }
    }

    if !is_python {
        return;
    }

    add_language(languages, Language::Python);

    if root.join("manage.py").exists() || packages.iter().any(|package| package == "django") {
        add_framework(frameworks, Framework::Django);
    }

    if packages.iter().any(|package| package == "fastapi") {
        add_framework(frameworks, Framework::FastApi);
    }
}

/// Extracts the normalized package name from a requirement spec such as
/// `Django>=4.2`, `fastapi[all]==0.110` or `requests ; python_version < "3.9"`.
/// Returns `None` for blank lines, comments and options (e.g. `-r other.txt`).
fn python_package_name(spec: &str) -> Option<String> {
    let spec = spec.trim();

    if spec.is_empty() || spec.starts_with('#') || spec.starts_with('-') {
        return None;
    }

    let name: String = spec
        .chars()
        .take_while(|character| {
            character.is_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
        .collect();

    if name.is_empty() {
        None
    } else {
        Some(name.to_lowercase())
    }
}

// Detect Rust
fn detect_rust(
    root: &Path,
    languages: &mut Vec<Language>,
    frameworks: &mut Vec<Framework>,
) {
    let path = root.join("Cargo.toml");

    if !path.exists() {
        return;
    }

    add_language(languages, Language::Rust);

    let Ok(manifest) = read_toml_file(&path) else {
        return;
    };

    let groups = [
        manifest.get("dependencies"),
        manifest.get("dev-dependencies"),
        manifest
            .get("workspace")
            .and_then(|workspace| workspace.get("dependencies")),
    ];

    for dependencies in groups
        .into_iter()
        .flatten()
        .filter_map(|group| group.as_table())
    {
        for package in dependencies.keys() {
            match package.as_str() {
                "axum" => {
                    add_framework(frameworks, Framework::Axum);
                }

                "actix-web" => {
                    add_framework(frameworks, Framework::Actix);
                }

                _ => {}
            }
        }
    }
}

// Detect JavaScript / TypeScript
fn detect_javascript(
    root: &Path,
    languages: &mut Vec<Language>,
    frameworks: &mut Vec<Framework>,
) {
    let path = root.join("package.json");

    if !path.exists() {
        return;
    }

    let mut is_typescript = root.join("tsconfig.json").exists();

    let manifest = match read_json_file(&path) {
        Ok(manifest) => manifest,
        Err(_) => {
            add_language(
                languages,
                if is_typescript {
                    Language::TypeScript
                } else {
                    Language::JavaScript
                },
            );
            return;
        }
    };

    for group in ["dependencies", "devDependencies"] {
        let Some(packages) = manifest
            .get(group)
            .and_then(|value| value.as_object())
        else {
            continue;
        };

        for package in packages.keys() {
            match package.as_str() {
                "typescript" => {
                    is_typescript = true;
                }

                "express" => {
                    add_framework(frameworks, Framework::Express);
                }

                "@nestjs/core" => {
                    add_framework(frameworks, Framework::NestJs);
                }

                _ => {}
            }
        }
    }

    add_language(
        languages,
        if is_typescript {
            Language::TypeScript
        } else {
            Language::JavaScript
        },
    );
}

// Detect Go
fn detect_go(
    root: &Path,
    languages: &mut Vec<Language>,
    _frameworks: &mut Vec<Framework>,
) {
    if root.join("go.mod").exists() {
        add_language(languages, Language::Go);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "docraly_test_discovery_{}_{}",
                name,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }

        fn write_file(&self, relative: &str, content: &str) {
            let full = self.path.join(relative);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full, content).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_python_package_name_parser() {
        assert_eq!(
            python_package_name("Django>=4.2,<5.0"),
            Some("django".to_string())
        );
        assert_eq!(
            python_package_name("fastapi[all]==0.110.0"),
            Some("fastapi".to_string())
        );
        assert_eq!(
            python_package_name("requests ; python_version < '3.9'"),
            Some("requests".to_string())
        );
        assert_eq!(python_package_name("# a comment"), None);
        assert_eq!(python_package_name("-r other-requirements.txt"), None);
        assert_eq!(python_package_name("   "), None);
    }

    #[test]
    fn test_detect_php_laravel_and_symfony() {
        let temp_laravel = TempDir::new("php_laravel");
        temp_laravel.write_file(
            "composer.json",
            r#"{
                "require": {
                    "php": "^8.2",
                    "laravel/framework": "^11.0"
                }
            }"#,
        );

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_php(&temp_laravel.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Php]);
        assert_eq!(frameworks, vec![Framework::Laravel]);

        let temp_symfony = TempDir::new("php_symfony");
        temp_symfony.write_file(
            "composer.json",
            r#"{
                "require": {
                    "symfony/framework-bundle": "^7.0"
                }
            }"#,
        );

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_php(&temp_symfony.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Php]);
        assert_eq!(frameworks, vec![Framework::Symfony]);
    }

    #[test]
    fn test_detect_python_fastapi_and_django() {
        // Test requirements.txt
        let temp_req = TempDir::new("py_req");
        temp_req.write_file("requirements.txt", "fastapi>=0.110.0\nuvicorn\n");

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_python(&temp_req.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Python]);
        assert_eq!(frameworks, vec![Framework::FastApi]);

        // Test pyproject.toml PEP 621
        let temp_pep = TempDir::new("py_pep");
        temp_pep.write_file(
            "pyproject.toml",
            r#"[project]
name = "my-app"
dependencies = [
    "django>=5.0",
]
"#,
        );

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_python(&temp_pep.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Python]);
        assert_eq!(frameworks, vec![Framework::Django]);

        // Test pyproject.toml Poetry
        let temp_poetry = TempDir::new("py_poetry");
        temp_poetry.write_file(
            "pyproject.toml",
            r#"[tool.poetry.dependencies]
python = "^3.11"
fastapi = "^0.110.0"
"#,
        );

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_python(&temp_poetry.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Python]);
        assert_eq!(frameworks, vec![Framework::FastApi]);

        // Test Pipfile
        let temp_pip = TempDir::new("py_pip");
        temp_pip.write_file(
            "Pipfile",
            r#"[packages]
django = "*"
"#,
        );

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_python(&temp_pip.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Python]);
        assert_eq!(frameworks, vec![Framework::Django]);
    }

    #[test]
    fn test_detect_rust_axum_and_actix() {
        let temp_axum = TempDir::new("rust_axum");
        temp_axum.write_file(
            "Cargo.toml",
            r#"[package]
name = "api"
version = "0.1.0"

[dependencies]
axum = "0.7"
"#,
        );

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_rust(&temp_axum.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Rust]);
        assert_eq!(frameworks, vec![Framework::Axum]);

        let temp_actix = TempDir::new("rust_actix");
        temp_actix.write_file(
            "Cargo.toml",
            r#"[package]
name = "api"
version = "0.1.0"

[dependencies]
actix-web = "4"
"#,
        );

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_rust(&temp_actix.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Rust]);
        assert_eq!(frameworks, vec![Framework::Actix]);
    }

    #[test]
    fn test_detect_javascript_and_typescript() {
        // JavaScript with Express
        let temp_js = TempDir::new("js_express");
        temp_js.write_file(
            "package.json",
            r#"{
                "dependencies": {
                    "express": "^4.19.0"
                }
            }"#,
        );

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_javascript(&temp_js.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::JavaScript]);
        assert_eq!(frameworks, vec![Framework::Express]);

        // TypeScript with NestJS
        let temp_ts = TempDir::new("ts_nest");
        temp_ts.write_file(
            "package.json",
            r#"{
                "dependencies": {
                    "@nestjs/core": "^10.0.0"
                },
                "devDependencies": {
                    "typescript": "^5.0.0"
                }
            }"#,
        );
        temp_ts.write_file("tsconfig.json", "{}");

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_javascript(&temp_ts.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::TypeScript]);
        assert_eq!(frameworks, vec![Framework::NestJs]);
    }

    #[test]
    fn test_detect_go() {
        let temp_go = TempDir::new("go_test");
        temp_go.write_file("go.mod", "module example.com/app\n\ngo 1.22\n");

        let mut languages = Vec::new();
        let mut frameworks = Vec::new();
        detect_go(&temp_go.path, &mut languages, &mut frameworks);
        assert_eq!(languages, vec![Language::Go]);
    }

    #[test]
    fn test_discover_project_polyglot() {
        let temp = TempDir::new("polyglot");
        temp.write_file(
            "composer.json",
            r#"{ "require": { "laravel/framework": "^11.0" } }"#,
        );
        temp.write_file(
            "package.json",
            r#"{ "dependencies": { "express": "^4.19.0" } }"#,
        );

        let project = discover_project(&temp.path);
        assert_eq!(project.languages, vec![Language::Php, Language::JavaScript]);
        assert_eq!(
            project.frameworks,
            vec![Framework::Laravel, Framework::Express]
        );
    }
}

