use std::path::Path;
use tree_sitter::{Node, Parser};

use super::FrameworkAdapter;
use crate::discovery::Framework;
use crate::model::{extract_path_parameters, ProjectModel, Route, RouteAction, SourceLocation};

pub struct LaravelAdapter;

impl LaravelAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LaravelAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameworkAdapter for LaravelAdapter {
    fn framework(&self) -> Framework {
        Framework::Laravel
    }

    fn analyze(&self, project: &mut ProjectModel) {
        // Collect indices of route files (files under routes/ or named like routes)
        let route_file_indices: Vec<usize> = project
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| is_route_file(&file.relative_path))
            .map(|(index, _)| index)
            .collect();

        for index in route_file_indices {
            let file = &project.files[index];
            if let Ok(source) = std::fs::read_to_string(&file.path) {
                let mut routes = parse_routes_from_source(&source);

                // Resolve controller short names to FQCN using file imports and namespace
                for route in &mut routes {
                    if let RouteAction::ControllerMethod {
                        controller_name,
                        controller_fqcn,
                        ..
                    } = &mut route.action
                    {
                        let resolved = file.resolve_class_name(controller_name);
                        *controller_fqcn = Some(resolved);
                    }
                }

                project.files[index].routes = routes;
            }
        }
    }
}

/// Checks whether a given relative or absolute path represents a Laravel route file.
pub fn is_route_file(path: &Path) -> bool {
    let clean = path.strip_prefix("./").unwrap_or(path);
    let path_str = clean.to_string_lossy();
    path_str.starts_with("routes/")
        || path_str.starts_with("routes\\")
        || path_str == "routes.php"
        || path_str.ends_with("/routes.php")
        || path.components().any(|c| c.as_os_str() == "routes")
}


/// Parses all Laravel route definitions from source code.
pub fn parse_routes_from_source(source: &str) -> Vec<Route> {
    let mut parser = Parser::new();
    let language = tree_sitter_php::LANGUAGE_PHP.into();

    if parser.set_language(&language).is_err() {
        return Vec::new();
    }

    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut routes = Vec::new();
    collect_routes_from_node(tree.root_node(), source, "", &[], "", None, &mut routes);
    routes
}

fn collect_routes_from_node(
    node: Node,
    source: &str,
    current_prefix: &str,
    current_middleware: &[String],
    current_name_prefix: &str,
    current_controller: Option<&str>,
    routes: &mut Vec<Route>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "expression_statement" => {
                if let Some(inner) = child.named_child(0) {
                    process_route_expression(
                        inner,
                        source,
                        current_prefix,
                        current_middleware,
                        current_name_prefix,
                        current_controller,
                        routes,
                    );
                }
            }
            _ => {
                // Check if this node itself is a route expression
                process_route_expression(
                    child,
                    source,
                    current_prefix,
                    current_middleware,
                    current_name_prefix,
                    current_controller,
                    routes,
                );
            }
        }
    }
}

fn process_route_expression(
    node: Node,
    source: &str,
    current_prefix: &str,
    current_middleware: &[String],
    current_name_prefix: &str,
    current_controller: Option<&str>,
    routes: &mut Vec<Route>,
) {
    if let Some(chain) = analyze_call_chain(node, source) {
        if chain.is_group {
            let next_prefix = match &chain.prefix {
                Some(p) => join_paths(current_prefix, p),
                None => current_prefix.to_string(),
            };

            let next_name_prefix = match &chain.name {
                Some(n) => format!("{}{}", current_name_prefix, n),
                None => current_name_prefix.to_string(),
            };

            let next_controller = chain.controller.as_deref().or(current_controller);

            let mut next_middleware = current_middleware.to_vec();
            for m in chain.middleware {
                if !next_middleware.contains(&m) {
                    next_middleware.push(m);
                }
            }

            if let Some(closure_node) = chain.group_closure {
                if let Some(body) = find_closure_body(closure_node) {
                    collect_routes_from_node(
                        body,
                        source,
                        &next_prefix,
                        &next_middleware,
                        &next_name_prefix,
                        next_controller,
                        routes,
                    );
                }
            }
        } else if !chain.verbs.is_empty() {
            let uri = join_paths(current_prefix, chain.uri.as_deref().unwrap_or(""));
            let mut middleware = current_middleware.to_vec();
            for m in chain.middleware {
                if !middleware.contains(&m) {
                    middleware.push(m);
                }
            }

            let path_parameters = extract_path_parameters(&uri);
            let route_name = chain.name.map(|n| format!("{}{}", current_name_prefix, n));

            let action = match chain.action {
                Some(RouteAction::View(ref method)) if current_controller.is_some() => {
                    Some(RouteAction::ControllerMethod {
                        controller_name: current_controller.unwrap().to_string(),
                        controller_fqcn: None,
                        method_name: method.clone(),
                    })
                }
                other => other,
            };

            if let Some(act) = action {
                for verb in chain.verbs {
                    routes.push(Route {
                        http_verb: verb,
                        uri: uri.clone(),
                        action: act.clone(),
                        name: route_name.clone(),
                        middleware: middleware.clone(),
                        path_parameters: path_parameters.clone(),
                        location: Some(node_location(node)),
                    });
                }
            }
        } else if chain.is_resource {
            let resource_name = chain.uri.as_deref().unwrap_or("");
            if let Some(RouteAction::ControllerMethod { controller_name, .. }) = chain.action {
                let base_uri = join_paths(current_prefix, resource_name);
                let last_segment = resource_name.rsplit('/').next().unwrap_or(resource_name);
                let param_name = singularize(last_segment);
                let mut middleware = current_middleware.to_vec();
                for m in chain.middleware {
                    if !middleware.contains(&m) {
                        middleware.push(m);
                    }
                }

                let clean_name_base = resource_name.replace('/', ".");
                let mut resource_actions = vec![
                    ("GET", base_uri.clone(), "index", format!("{}{}.index", current_name_prefix, clean_name_base)),
                ];
                if !chain.is_api_resource {
                    resource_actions.push((
                        "GET",
                        format!("{}/create", base_uri),
                        "create",
                        format!("{}{}.create", current_name_prefix, clean_name_base),
                    ));
                }
                resource_actions.push((
                    "POST",
                    base_uri.clone(),
                    "store",
                    format!("{}{}.store", current_name_prefix, clean_name_base),
                ));
                resource_actions.push((
                    "GET",
                    format!("{}/{{{}}}", base_uri, param_name),
                    "show",
                    format!("{}{}.show", current_name_prefix, clean_name_base),
                ));
                if !chain.is_api_resource {
                    resource_actions.push((
                        "GET",
                        format!("{}/{{{}}}/edit", base_uri, param_name),
                        "edit",
                        format!("{}{}.edit", current_name_prefix, clean_name_base),
                    ));
                }
                resource_actions.push((
                    "PUT",
                    format!("{}/{{{}}}", base_uri, param_name),
                    "update",
                    format!("{}{}.update", current_name_prefix, clean_name_base),
                ));
                resource_actions.push((
                    "DELETE",
                    format!("{}/{{{}}}", base_uri, param_name),
                    "destroy",
                    format!("{}{}.destroy", current_name_prefix, clean_name_base),
                ));

                for (verb, uri, method_name, name) in resource_actions {
                    let path_parameters = extract_path_parameters(&uri);
                    routes.push(Route {
                        http_verb: verb.to_string(),
                        uri,
                        action: RouteAction::ControllerMethod {
                            controller_name: controller_name.clone(),
                            controller_fqcn: None,
                            method_name: method_name.to_string(),
                        },
                        name: Some(name),
                        middleware: middleware.clone(),
                        path_parameters,
                        location: Some(node_location(node)),
                    });
                }
            }
        }
    }
}

#[derive(Default)]
struct RouteCallChain<'a> {
    verbs: Vec<String>,
    uri: Option<String>,
    action: Option<RouteAction>,
    name: Option<String>,
    middleware: Vec<String>,
    prefix: Option<String>,
    controller: Option<String>,
    is_group: bool,
    group_closure: Option<Node<'a>>,
    is_resource: bool,
    is_api_resource: bool,
}

fn analyze_call_chain<'a>(node: Node<'a>, source: &str) -> Option<RouteCallChain<'a>> {
    let mut chain = RouteCallChain::default();
    let mut current = node;

    // Traverse member call chains like `Route::get(...)->name(...)->middleware(...)`
    while current.kind() == "member_call_expression" {
        let mut cursor = current.walk();
        let named_children: Vec<_> = current.named_children(&mut cursor).collect();

        if named_children.len() >= 2 {
            let method_node = named_children[1];
            let method_name = child_text(method_node, source);
            let args_node = current.child_by_field_name("arguments");

            match method_name.as_str() {
                "name" | "as" => {
                    if let Some(args) = args_node {
                        if let Some(first_arg) = first_argument(args) {
                            chain.name = Some(unquote(&child_text(first_arg, source)));
                        }
                    }
                }
                "middleware" => {
                    if let Some(args) = args_node {
                        for arg in extract_string_arguments(args, source) {
                            if !chain.middleware.contains(&arg) {
                                chain.middleware.push(arg);
                            }
                        }
                    }
                }
                "prefix" => {
                    if let Some(args) = args_node {
                        if let Some(first_arg) = first_argument(args) {
                            chain.prefix = Some(unquote(&child_text(first_arg, source)));
                        }
                    }
                }
                "controller" => {
                    if let Some(args) = args_node {
                        if let Some(first_arg) = first_argument(args) {
                            chain.controller = extract_class_from_node(first_arg, source);
                        }
                    }
                }
                "group" => {
                    chain.is_group = true;
                    if let Some(args) = args_node {
                        let arg_list = argument_list(args);
                        if arg_list.len() == 1 {
                            chain.group_closure = Some(arg_list[0]);
                        } else if arg_list.len() >= 2 {
                            extract_group_attributes(arg_list[0], source, &mut chain);
                            chain.group_closure = Some(arg_list[1]);
                        }
                    }
                }
                _ => {}
            }

            // Move to the object of the member call
            current = named_children[0];
        } else {
            break;
        }
    }

    // Now current should be the base scoped_call_expression (e.g. `Route::get(...)`)
    if current.kind() == "scoped_call_expression" {
        let mut cursor = current.walk();
        let named_children: Vec<_> = current.named_children(&mut cursor).collect();

        if named_children.len() >= 2 {
            let scope_name = child_text(named_children[0], source);
            let method_name = child_text(named_children[1], source);

            if is_route_facade(&scope_name) {
                let args_node = current.child_by_field_name("arguments");

                match method_name.as_str() {
                    "get" | "post" | "put" | "patch" | "delete" | "options" | "any" => {
                        chain.verbs = vec![method_name.to_uppercase()];

                        if let Some(args) = args_node {
                            let arg_list = argument_list(args);
                            if let Some(first) = arg_list.first() {
                                chain.uri = Some(unquote(&child_text(*first, source)));
                            }
                            if let Some(second) = arg_list.get(1) {
                                chain.action = extract_route_action(*second, source);
                            }
                        }
                        return Some(chain);
                    }
                    "match" => {
                        if let Some(args) = args_node {
                            let arg_list = argument_list(args);
                            if let Some(first) = arg_list.first() {
                                chain.verbs = extract_verbs_from_node(*first, source);
                            }
                            if let Some(second) = arg_list.get(1) {
                                chain.uri = Some(unquote(&child_text(*second, source)));
                            }
                            if let Some(third) = arg_list.get(2) {
                                chain.action = extract_route_action(*third, source);
                            }
                        }
                        return Some(chain);
                    }
                    "fallback" => {
                        chain.verbs = vec!["ANY".to_string()];
                        chain.uri = Some("{fallbackPlaceholder}".to_string());
                        if let Some(args) = args_node {
                            let arg_list = argument_list(args);
                            if let Some(first) = arg_list.first() {
                                chain.action = extract_route_action(*first, source);
                            }
                        }
                        return Some(chain);
                    }
                    "apiResource" => {
                        chain.is_resource = true;
                        chain.is_api_resource = true;
                        if let Some(args) = args_node {
                            let arg_list = argument_list(args);
                            if let Some(first) = arg_list.first() {
                                chain.uri = Some(unquote(&child_text(*first, source)));
                            }
                            if let Some(second) = arg_list.get(1) {
                                chain.action = extract_route_action(*second, source);
                            }
                        }
                        return Some(chain);
                    }
                    "resource" => {
                        chain.is_resource = true;
                        chain.is_api_resource = false;
                        if let Some(args) = args_node {
                            let arg_list = argument_list(args);
                            if let Some(first) = arg_list.first() {
                                chain.uri = Some(unquote(&child_text(*first, source)));
                            }
                            if let Some(second) = arg_list.get(1) {
                                chain.action = extract_route_action(*second, source);
                            }
                        }
                        return Some(chain);
                    }
                    "view" => {
                        chain.verbs = vec!["GET".to_string()];
                        if let Some(args) = args_node {
                            let arg_list = argument_list(args);
                            if let Some(first) = arg_list.first() {
                                chain.uri = Some(unquote(&child_text(*first, source)));
                            }
                            if let Some(second) = arg_list.get(1) {
                                chain.action = Some(RouteAction::View(unquote(&child_text(*second, source))));
                            }
                        }
                        return Some(chain);
                    }
                    "prefix" => {
                        if let Some(args) = args_node {
                            if let Some(first) = first_argument(args) {
                                chain.prefix = Some(unquote(&child_text(first, source)));
                            }
                        }
                        return Some(chain);
                    }
                    "middleware" => {
                        if let Some(args) = args_node {
                            for arg in extract_string_arguments(args, source) {
                                if !chain.middleware.contains(&arg) {
                                    chain.middleware.push(arg);
                                }
                            }
                        }
                        return Some(chain);
                    }
                    "controller" => {
                        if let Some(args) = args_node {
                            if let Some(first_arg) = first_argument(args) {
                                chain.controller = extract_class_from_node(first_arg, source);
                            }
                        }
                        return Some(chain);
                    }
                    "name" | "as" => {
                        if let Some(args) = args_node {
                            if let Some(first_arg) = first_argument(args) {
                                chain.name = Some(unquote(&child_text(first_arg, source)));
                            }
                        }
                        return Some(chain);
                    }
                    "group" => {
                        chain.is_group = true;
                        if let Some(args) = args_node {
                            let arg_list = argument_list(args);
                            if arg_list.len() == 1 {
                                chain.group_closure = Some(arg_list[0]);
                            } else if arg_list.len() >= 2 {
                                extract_group_attributes(arg_list[0], source, &mut chain);
                                chain.group_closure = Some(arg_list[1]);
                            }
                        }
                        return Some(chain);
                    }
                    _ => {}
                }
            }
        }
    }

    if chain.is_group || !chain.verbs.is_empty() || chain.is_resource {
        Some(chain)
    } else {
        None
    }
}


fn is_route_facade(scope: &str) -> bool {
    let trimmed = scope.trim().trim_start_matches('\\');
    trimmed == "Route" || trimmed == "Illuminate\\Support\\Facades\\Route"
}

fn extract_route_action(node: Node, source: &str) -> Option<RouteAction> {
    match node.kind() {
        // [UserController::class, 'index']
        "array_creation_expression" => {
            let mut cursor = node.walk();
            let elements: Vec<_> = node
                .named_children(&mut cursor)
                .filter(|child| child.kind() == "array_element_initializer")
                .collect();

            if elements.len() >= 2 {
                let class_elem = elements[0];
                let method_elem = elements[1];

                let controller_name = extract_class_from_element(class_elem, source)?;
                let method_name = extract_string_from_element(method_elem, source)?;

                Some(RouteAction::ControllerMethod {
                    controller_name,
                    controller_fqcn: None,
                    method_name,
                })
            } else {
                None
            }
        }
        // UserController::class (for apiResource/resource or invokable controllers)
        "class_constant_access_expression" => {
            let mut cursor = node.walk();
            let parts: Vec<_> = node.named_children(&mut cursor).collect();
            if parts.len() >= 2 && child_text(parts[1], source) == "class" {
                let controller = child_text(parts[0], source).trim().to_string();
                Some(RouteAction::ControllerMethod {
                    controller_name: controller,
                    controller_fqcn: None,
                    method_name: "__invoke".to_string(),
                })
            } else {
                None
            }
        }
        // 'UserController@index' or 'App\Http\Controllers\UserController@index'
        "string" | "encapsed_string" => {
            let text = unquote(&child_text(node, source));
            if let Some((controller, method)) = text.split_once('@') {
                Some(RouteAction::ControllerMethod {
                    controller_name: controller.to_string(),
                    controller_fqcn: None,
                    method_name: method.to_string(),
                })
            } else {
                Some(RouteAction::View(text))
            }
        }
        // function () { ... } or fn () => ...
        "anonymous_function" | "anonymous_function_creation_expression" | "arrow_function" => {
            Some(RouteAction::Closure)
        }
        _ => None,
    }
}


fn extract_class_from_element(element: Node, source: &str) -> Option<String> {
    let mut cursor = element.walk();
    for child in element.named_children(&mut cursor) {
        if child.kind() == "class_constant_access_expression" {
            let mut sub_cursor = child.walk();
            let parts: Vec<_> = child.named_children(&mut sub_cursor).collect();
            if parts.len() >= 2 {
                let const_name = child_text(parts[1], source);
                if const_name == "class" {
                    return Some(child_text(parts[0], source).trim().to_string());
                }
            }
        } else if child.kind() == "string" {
            return Some(unquote(&child_text(child, source)));
        }
    }
    None
}

fn extract_string_from_element(element: Node, source: &str) -> Option<String> {
    let mut cursor = element.walk();
    for child in element.named_children(&mut cursor) {
        if child.kind() == "string" {
            return Some(unquote(&child_text(child, source)));
        }
    }
    None
}

fn find_closure_body<'a>(closure: Node<'a>) -> Option<Node<'a>> {
    let mut cursor = closure.walk();
    closure
        .named_children(&mut cursor)
        .find(|&child| child.kind() == "compound_statement")
}

fn extract_group_attributes(node: Node, source: &str, chain: &mut RouteCallChain) {
    if node.kind() == "array_creation_expression" {
        let mut cursor = node.walk();
        for elem in node.named_children(&mut cursor) {
            if elem.kind() == "array_element_initializer" {
                let mut elem_cursor = elem.walk();
                let children: Vec<_> = elem.named_children(&mut elem_cursor).collect();
                if children.len() >= 2 {
                    let key = unquote(&child_text(children[0], source));
                    let val_node = children[1];
                    match key.as_str() {
                        "prefix" => {
                            chain.prefix = Some(unquote(&child_text(val_node, source)));
                        }
                        "as" | "name" => {
                            chain.name = Some(unquote(&child_text(val_node, source)));
                        }
                        "controller" => {
                            chain.controller = extract_class_from_node(val_node, source);
                        }
                        "middleware" => match val_node.kind() {
                            "string" => {
                                let mw = unquote(&child_text(val_node, source));
                                if !chain.middleware.contains(&mw) {
                                    chain.middleware.push(mw);
                                }
                            }
                            "array_creation_expression" => {
                                let mut arr_cursor = val_node.walk();
                                for item in val_node.named_children(&mut arr_cursor) {
                                    if item.kind() == "array_element_initializer" {
                                        let mut it_cursor = item.walk();
                                        for it_child in item.named_children(&mut it_cursor) {
                                            if it_child.kind() == "string" {
                                                let mw = unquote(&child_text(it_child, source));
                                                if !chain.middleware.contains(&mw) {
                                                    chain.middleware.push(mw);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
        }
    }
}

fn extract_verbs_from_node(node: Node, source: &str) -> Vec<String> {
    let mut verbs = Vec::new();
    match node.kind() {
        "string" => {
            verbs.push(unquote(&child_text(node, source)).to_uppercase());
        }
        "array_creation_expression" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "array_element_initializer" {
                    let mut item_cursor = child.walk();
                    for item in child.named_children(&mut item_cursor) {
                        if item.kind() == "string" {
                            verbs.push(unquote(&child_text(item, source)).to_uppercase());
                        }
                    }
                }
            }
        }
        _ => {}
    }
    verbs
}

fn singularize(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.ends_with("ies") && trimmed.len() > 3 {
        format!("{}y", &trimmed[..trimmed.len() - 3])
    } else if trimmed.ends_with("sses") && trimmed.len() > 4 {
        trimmed[..trimmed.len() - 2].to_string()
    } else if trimmed.ends_with("us") || trimmed.ends_with("is") || trimmed.ends_with("ss") {
        trimmed.to_string()
    } else if trimmed.ends_with('s') && trimmed.len() > 1 {
        trimmed[..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}


fn extract_class_from_node(node: Node, source: &str) -> Option<String> {
    match node.kind() {
        "class_constant_access_expression" => {
            let mut cursor = node.walk();
            let parts: Vec<_> = node.named_children(&mut cursor).collect();
            if parts.len() >= 2 && child_text(parts[1], source) == "class" {
                Some(child_text(parts[0], source).trim().to_string())
            } else {
                None
            }
        }
        "string" | "encapsed_string" => Some(unquote(&child_text(node, source))),
        "name" | "qualified_name" => Some(child_text(node, source).trim().to_string()),
        _ => None,
    }
}

fn argument_list<'a>(args_node: Node<'a>) -> Vec<Node<'a>> {
    let mut cursor = args_node.walk();
    args_node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "argument")
        .filter_map(|arg| arg.named_child(0))
        .collect()
}

fn first_argument<'a>(args_node: Node<'a>) -> Option<Node<'a>> {
    argument_list(args_node).into_iter().next()
}

fn extract_string_arguments(args_node: Node, source: &str) -> Vec<String> {
    let mut result = Vec::new();
    for arg in argument_list(args_node) {
        match arg.kind() {
            "string" => {
                result.push(unquote(&child_text(arg, source)));
            }
            "array_creation_expression" => {
                let mut cursor = arg.walk();
                for elem in arg.named_children(&mut cursor) {
                    if elem.kind() == "array_element_initializer" {
                        let mut elem_cursor = elem.walk();
                        for child in elem.named_children(&mut elem_cursor) {
                            if child.kind() == "string" {
                                result.push(unquote(&child_text(child, source)));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    result
}

fn unquote(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('"') && trimmed.ends_with('"')))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}


fn join_paths(prefix: &str, path: &str) -> String {
    let p = prefix.trim_matches('/');
    let s = path.trim_matches('/');
    if p.is_empty() && s.is_empty() {
        "/".to_string()
    } else if p.is_empty() {
        format!("/{}", s)
    } else if s.is_empty() {
        format!("/{}", p)
    } else {
        format!("/{}/{}", p, s)
    }
}

fn node_location(node: Node) -> SourceLocation {
    SourceLocation {
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    }
}

fn child_text(node: Node, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_get_post_routes() {
        let code = r#"<?php
use App\Http\Controllers\UserController;
use Illuminate\Support\Facades\Route;

Route::get('/users', [UserController::class, 'index'])->name('users.index');
Route::post('/users', [UserController::class, 'store'])->middleware('auth');
Route::delete('/users/{id}', [UserController::class, 'destroy']);
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 3);

        assert_eq!(routes[0].http_verb, "GET");
        assert_eq!(routes[0].uri, "/users");
        assert_eq!(routes[0].name, Some("users.index".to_string()));
        assert_eq!(
            routes[0].action,
            RouteAction::ControllerMethod {
                controller_name: "UserController".to_string(),
                controller_fqcn: None,
                method_name: "index".to_string(),
            }
        );

        assert_eq!(routes[1].http_verb, "POST");
        assert_eq!(routes[1].middleware, vec!["auth"]);

        assert_eq!(routes[2].http_verb, "DELETE");
        assert_eq!(routes[2].uri, "/users/{id}");
        assert_eq!(routes[2].path_parameters, vec!["id"]);
    }

    #[test]
    fn test_parse_route_group_and_prefix() {
        let code = r#"<?php
use App\Http\Controllers\Admin\DashboardController;
use Illuminate\Support\Facades\Route;

Route::prefix('admin')->middleware(['auth', 'verified'])->group(function () {
    Route::get('/dashboard', [DashboardController::class, 'index'])->name('admin.dashboard');
});
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 1);

        assert_eq!(routes[0].http_verb, "GET");
        assert_eq!(routes[0].uri, "/admin/dashboard");
        assert_eq!(routes[0].name, Some("admin.dashboard".to_string()));
        assert_eq!(routes[0].middleware, vec!["auth", "verified"]);
    }

    #[test]
    fn test_parse_api_resource() {
        let code = r#"<?php
use App\Http\Controllers\PostController;
use Illuminate\Support\Facades\Route;

Route::apiResource('posts', PostController::class);
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 5);

        let verbs: Vec<_> = routes.iter().map(|r| r.http_verb.as_str()).collect();
        assert_eq!(verbs, vec!["GET", "POST", "GET", "PUT", "DELETE"]);

        assert_eq!(routes[0].uri, "/posts");
        assert_eq!(routes[2].uri, "/posts/{post}");
        assert_eq!(routes[2].path_parameters, vec!["post"]);
    }

    #[test]
    fn test_parse_closure_route() {
        let code = r#"<?php
use Illuminate\Support\Facades\Route;

Route::get('/health', function () {
    return response()->json(['status' => 'ok']);
});
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].http_verb, "GET");
        assert_eq!(routes[0].uri, "/health");
        assert_eq!(routes[0].action, RouteAction::Closure);
    }

    #[test]
    fn test_parse_route_group_array_syntax() {
        let code = r#"<?php
use App\Http\Controllers\ItemController;
use Illuminate\Support\Facades\Route;

Route::group(['prefix' => 'api/v1', 'middleware' => ['api', 'auth']], function () {
    Route::get('/items', [ItemController::class, 'index']);
});
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].http_verb, "GET");
        assert_eq!(routes[0].uri, "/api/v1/items");
        assert_eq!(routes[0].middleware, vec!["api", "auth"]);
    }

    #[test]
    fn test_parse_full_resource() {
        let code = r#"<?php
use App\Http\Controllers\ArticleController;
use Illuminate\Support\Facades\Route;

Route::resource('articles', ArticleController::class);
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 7);
        let methods: Vec<_> = routes
            .iter()
            .map(|r| match &r.action {
                RouteAction::ControllerMethod { method_name, .. } => method_name.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(
            methods,
            vec!["index", "create", "store", "show", "edit", "update", "destroy"]
        );
    }

    #[test]
    fn test_parse_route_view() {
        let code = r#"<?php
use Illuminate\Support\Facades\Route;

Route::view('/about', 'pages.about');
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].http_verb, "GET");
        assert_eq!(routes[0].uri, "/about");
        assert_eq!(routes[0].action, RouteAction::View("pages.about".to_string()));
    }

    #[test]
    fn test_parse_route_controller_group() {
        let code = r#"<?php
use App\Http\Controllers\OrderController;
use Illuminate\Support\Facades\Route;

Route::controller(OrderController::class)->group(function () {
    Route::get('/orders', 'index')->name('orders.index');
    Route::post('/orders', 'store');
});
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].http_verb, "GET");
        assert_eq!(routes[0].uri, "/orders");
        assert_eq!(routes[0].name, Some("orders.index".to_string()));
        assert_eq!(
            routes[0].action,
            RouteAction::ControllerMethod {
                controller_name: "OrderController".to_string(),
                controller_fqcn: None,
                method_name: "index".to_string(),
            }
        );
        assert_eq!(routes[1].http_verb, "POST");
        assert_eq!(
            routes[1].action,
            RouteAction::ControllerMethod {
                controller_name: "OrderController".to_string(),
                controller_fqcn: None,
                method_name: "store".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_nested_group_name_and_prefix() {
        let code = r#"<?php
use App\Http\Controllers\Admin\UserController;
use Illuminate\Support\Facades\Route;

Route::prefix('admin')->name('admin.')->group(function () {
    Route::get('/users', [UserController::class, 'index'])->name('users.index');
});
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].http_verb, "GET");
        assert_eq!(routes[0].uri, "/admin/users");
        assert_eq!(routes[0].name, Some("admin.users.index".to_string()));
    }

    #[test]
    fn test_parse_route_match_and_fallback() {
        let code = r#"<?php
use App\Http\Controllers\FeedbackController;
use Illuminate\Support\Facades\Route;

Route::match(['GET', 'POST'], '/feedback', [FeedbackController::class, 'handle']);
Route::fallback([FeedbackController::class, 'notFound']);
"#;
        let routes = parse_routes_from_source(code);
        assert_eq!(routes.len(), 3);
        assert_eq!(routes[0].http_verb, "GET");
        assert_eq!(routes[0].uri, "/feedback");
        assert_eq!(routes[1].http_verb, "POST");
        assert_eq!(routes[1].uri, "/feedback");
        assert_eq!(routes[2].http_verb, "ANY");
        assert_eq!(routes[2].uri, "/{fallbackPlaceholder}");
    }


    #[test]
    fn test_unquote_safety() {
        assert_eq!(unquote(""), "");
        assert_eq!(unquote("'"), "'");
        assert_eq!(unquote("\""), "\"");
        assert_eq!(unquote("''"), "");
        assert_eq!(unquote("\"\""), "");
        assert_eq!(unquote("'hello'"), "hello");
    }

    #[test]
    fn test_singularize_resource() {
        assert_eq!(singularize("users"), "user");
        assert_eq!(singularize("categories"), "category");
        assert_eq!(singularize("addresses"), "address");
        assert_eq!(singularize("status"), "status");
    }
}


