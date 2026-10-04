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
