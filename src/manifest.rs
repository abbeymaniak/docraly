use std::fs;
use std::path::Path;

pub fn read_json_file(path: &Path) -> Result<serde_json::Value, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;

    serde_json::from_str(&contents)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))
}

pub fn read_toml_file(path: &Path) -> Result<toml::Value, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;

    contents
        .parse::<toml::Value>()
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))
}