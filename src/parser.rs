use tree_sitter::Parser;
use tree_sitter_php;
use std::fs;
use std::error::Error;

pub fn parse_php(source: &str) -> Result<tree_sitter::Tree, String> {
    let mut parser = Parser::new();

    let language = tree_sitter_php::LANGUAGE_PHP.into();

    parser
        .set_language(&language)
        .map_err(|error| format!("Failed to load PHP parser: {error}"))?;

    parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse PHP source".to_string())
}