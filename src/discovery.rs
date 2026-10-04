use std::path::{Path, PathBuf};

use crate::manifest::{read_json_file, read_toml_file};

#[derive(Debug)]
pub struct ProjectInfo {
    pub root: PathBuf,
    pub languages: Vec<Language>,
    pub frameworks: Vec<Framework>,
}

#[derive(Debug)]
pub enum Language {
    Php,
    Python,
    Rust,
    JavaScript,
    TypeScript,
    Go,
}

#[derive(Debug)]
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
fn detect_php(root: &Path, languages: &mut Vec<Language>, frameworks: &mut Vec<Framework>) {
    let composer_path = root.join("composer.json");

    if !composer_path.exists() && !root.join("composer.lock").exists() {
        return;
    }

    languages.push(Language::Php);

    let contents = std::fs::read_to_string(composer_path).unwrap_or_default();

    if root.join("artisan").exists() || contents.contains("laravel/framework") {
        frameworks.push(Framework::Laravel);
    }

    if root.join("bin/console").exists() || contents.contains("symfony/") {
        frameworks.push(Framework::Symfony);
    }
}

// Detect Python
fn detect_python(root: &Path, languages: &mut Vec<Language>, frameworks: &mut Vec<Framework>) {
    let mut is_python = false;
    let mut manifest_contents = String::new();

    for manifest in ["requirements.txt", "pyproject.toml", "Pipfile"] {
        let path = root.join(manifest);
        if let Ok(contents) = std::fs::read_to_string(path) {
            is_python = true;
            manifest_contents.push_str(&contents.to_lowercase());
        }
    }

    if root.join("manage.py").exists() {
        is_python = true;
    }

    if !is_python {
        return;
    }

    languages.push(Language::Python);

    if root.join("manage.py").exists() || manifest_contents.contains("django") {
        frameworks.push(Framework::Django);
    }

    if manifest_contents.contains("fastapi") {
        frameworks.push(Framework::FastApi);
    }
}


//RUST

fn detect_rust(
    root: &Path,
    languages: &mut Vec<Language>,
    frameworks: &mut Vec<Framework>,
) {
    let cargo = root.join("Cargo.toml");

    if !cargo.exists() {
        return;
    }

    languages.push(Language::Rust);

    let contents = std::fs::read_to_string(cargo).unwrap_or_default();

    if contents.contains("axum") {
        frameworks.push(Framework::Axum);
    }

    if contents.contains("actix-web") {
        frameworks.push(Framework::Actix);
    }
}

//decect Javascript
fn detect_javascript(
    root: &Path,
    languages: &mut Vec<Language>,
    frameworks: &mut Vec<Framework>,
) {
    let package_json = root.join("package.json");

    if !package_json.exists() {
        return;
    }

    let contents = std::fs::read_to_string(package_json).unwrap_or_default();

    if contents.contains("typescript") {
        languages.push(Language::TypeScript);
    } else {
        languages.push(Language::JavaScript);
    }

    if contents.contains("\"express\"") {
        frameworks.push(Framework::Express);
    }

    if contents.contains("\"@nestjs/core\"") {
        frameworks.push(Framework::NestJs);
    }
}


fn detect_go(
    root: &Path,
    languages: &mut Vec<Language>,
    _frameworks: &mut Vec<Framework>,
) {
    if root.join("go.mod").exists() {
        languages.push(Language::Go);
    }
}
