use crate::model::{Function, Parameter, Type, Class, Method};
use tree_sitter::{Node, Parser};

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

pub fn find_functions(node: Node, source: &str) -> Vec<Function> {
    let mut functions = Vec::new();

    collect_functions(node, source, &mut functions);

    functions
}

fn collect_functions(
    node: Node,
    source: &str,
    functions: &mut Vec<Function>,
) {
    if node.kind() == "function_definition" {
        if let Some(function) = extract_function(node, source) {
            functions.push(function);
        }
    }

    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        collect_functions(child, source, functions);
    }
}

fn extract_function(node: Node, source: &str) -> Option<Function> {
    let mut name = String::new();
    let mut parameters = Vec::new();
    let mut return_type = None;

    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        // println!("CHILD: {}", child.kind());
        match child.kind() {
            "name" => {
                name = child_text(child, source);
            }

            "formal_parameters" => {
                parameters = extract_parameters(child, source);
            }

           "primitive_type" | "named_type" => {
    let text = child_text(child, source)
        .trim()
        .to_string();

    if !text.is_empty() {
        return_type = Some(Type {
            nullable: text.starts_with('?'),
            name: text.trim_start_matches('?').to_string(),
        });
    }
}


            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Function {
        name,
        parameters,
        return_type,
    })
}

fn extract_parameters(node: Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();

    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        if child.kind() == "simple_parameter" {
            parameters.push(extract_parameter(child, source));
        }
    }

    parameters
}

fn extract_parameter(node: Node, source: &str) -> Parameter {
    let mut name = String::new();
    let mut type_name: Option<Type> = None;

    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "variable_name" => {
                name = child_text(child, source);
            }

           "type" | "primitive_type" => {
                let text = child_text(child, source);
                type_name = Some(Type {
                    nullable: text.starts_with('?'),
                    name: text.trim_start_matches('?').to_string(),
                });
            }
            _ => {}
        }
    }

    Parameter {
        name,
        type_name,
    }
}

fn child_text(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or("")
        .to_string()
}

pub fn find_classes(node: Node, source: &str) -> Vec<Class> {
    let mut classes = Vec::new();

    collect_classes(node, source, &mut classes);

    classes
}

fn collect_classes(
    node: Node,
    source: &str,
    classes: &mut Vec<Class>,
) {
    if node.kind() == "class_declaration" {
        if let Some(class) = extract_class(node, source) {
            classes.push(class);
        }
    }

    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        collect_classes(child, source, classes);
    }
}

fn extract_class(node: Node, source: &str) -> Option<Class> {
    let mut name = String::new();

    //Todo: we'll eventually build a proper file-level representation to get namespace.
    // let mut namespace = None;
    let mut methods = Vec::new();

    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "name" => {
                name = child_text(child, source);
            }

            "declaration_list" => {
                methods = extract_methods(child, source);
            }

            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Class {
        name,
        namespace: None,
        methods,
    })
}


fn extract_methods(node: Node, source: &str) -> Vec<Method> {
    let mut methods = Vec::new();

    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        if child.kind() == "method_declaration" {
            if let Some(method) = extract_method(child, source) {
                methods.push(method);
            }
        }
    }

    methods
}

fn extract_method(node: Node, source: &str) -> Option<Method> {
    let mut name = String::new();
    let mut parameters = Vec::new();
    let mut return_type = None;

    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "name" => {
                name = child_text(child, source);
            }

            "formal_parameters" => {
                parameters = extract_parameters(child, source);
            }

            "primitive_type" | "named_type" => {
                let text = child_text(child, source)
                    .trim()
                    .to_string();

                if !text.is_empty() {
                    return_type = Some(Type {
                        nullable: text.starts_with('?'),
                        name: text.trim_start_matches('?').to_string(),
                    });
                }
            }

            _ => {}
        }
    }

    if name.is_empty() {
        return None;
    }

    Some(Method {
        name,
        parameters,
        return_type,
    })
}

