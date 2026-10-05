use std::path::PathBuf;

/// Represents a source location in a file (1-indexed line numbers and byte offsets).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Represents an import or use statement (e.g. `use App\Models\User as UserModel;`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub path: String,
    pub alias: Option<String>,
}

/// Represents the action executed when a route is matched.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteAction {
    ControllerMethod {
        controller_name: String,
        controller_fqcn: Option<String>,
        method_name: String,
    },
    Closure,
    View(String),
}

/// Represents an HTTP route defined in the application.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub http_verb: String,
    pub uri: String,
    pub action: RouteAction,
    pub name: Option<String>,
    pub middleware: Vec<String>,
    pub path_parameters: Vec<String>,
    pub location: Option<SourceLocation>,
}

/// Extracts `{param}` or `{param?}` path parameters from a route URI.
#[allow(dead_code)]
pub fn extract_path_parameters(uri: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut in_param = false;
    let mut current = String::new();

    for ch in uri.chars() {
        if ch == '{' {
            in_param = true;
            current.clear();
        } else if ch == '}' {
            if in_param && !current.is_empty() {
                let clean = current.trim_end_matches('?');
                let param_name = clean.split(':').next().unwrap_or(clean).to_string();
                if !params.contains(&param_name) {
                    params.push(param_name);
                }
            }
            in_param = false;
        } else if in_param {
            current.push(ch);
        }
    }

    params
}


/// Represents the analyzed model of a single source file.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileModel {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub namespace: Option<String>,
    pub imports: Vec<Import>,
    pub functions: Vec<Function>,
    pub classes: Vec<Class>,
    pub routes: Vec<Route>,
}

#[allow(dead_code)]
impl FileModel {
    pub fn new(path: PathBuf, relative_path: PathBuf) -> Self {
        Self {
            path,
            relative_path,
            namespace: None,
            imports: Vec::new(),
            functions: Vec::new(),
            classes: Vec::new(),
            routes: Vec::new(),
        }
    }

    pub fn find_class(&self, name: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.name == name)
    }

    /// Resolves a short or relative class name into a fully-qualified class name (FQCN)
    /// using the file's imports and namespace declarations.
    pub fn resolve_class_name(&self, name: &str) -> String {
        let trimmed = name.trim();
        if trimmed.starts_with('\\') {
            return trimmed.trim_start_matches('\\').to_string();
        }

        // If it contains a backslash, check if the first segment is an import or alias
        if let Some((first_segment, rest)) = trimmed.split_once('\\') {
            for import in &self.imports {
                if let Some(alias) = &import.alias {
                    if alias == first_segment {
                        return format!("{}\\{}", import.path, rest);
                    }
                } else {
                    let last_segment = import.path.rsplit('\\').next().unwrap_or(&import.path);
                    if last_segment == first_segment {
                        return format!("{}\\{}", import.path, rest);
                    }
                }
            }

            // If it already looks like a root namespace like App\... or Database\...
            if trimmed.starts_with("App\\") || trimmed.starts_with("Database\\") {
                return trimmed.to_string();
            }
        }

        // Check if trimmed matches an import alias or the final segment of an import path
        for import in &self.imports {
            if let Some(alias) = &import.alias {
                if alias == trimmed {
                    return import.path.clone();
                }
            } else {
                let last_segment = import.path.rsplit('\\').next().unwrap_or(&import.path);
                if last_segment == trimmed {
                    return import.path.clone();
                }
            }
        }

        // If the file has a namespace, qualify relative to it
        match &self.namespace {
            Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, trimmed),
            _ => trimmed.to_string(),
        }
    }
}

/// Represents the analyzed code model across the entire project.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectModel {
    pub files: Vec<FileModel>,
}

#[allow(dead_code)]
impl ProjectModel {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn all_classes(&self) -> impl Iterator<Item = &Class> {
        self.files.iter().flat_map(|f| &f.classes)
    }

    pub fn all_functions(&self) -> impl Iterator<Item = &Function> {
        self.files.iter().flat_map(|f| &f.functions)
    }

    pub fn all_routes(&self) -> impl Iterator<Item = &Route> {
        self.files.iter().flat_map(|f| &f.routes)
    }

    pub fn find_class_by_fqcn(&self, fqcn: &str) -> Option<&Class> {
        self.all_classes().find(|c| c.fully_qualified_name() == fqcn)
    }

    pub fn find_class(&self, file: &FileModel, name: &str) -> Option<&Class> {
        let fqcn = file.resolve_class_name(name);
        self.find_class_by_fqcn(&fqcn)
    }

    pub fn link_route_to_controller(&self, route: &Route) -> Option<(&Class, &Method)> {
        if let RouteAction::ControllerMethod {
            controller_fqcn: Some(fqcn),
            method_name,
            ..
        } = &route.action
        {
            let class = self.find_class_by_fqcn(fqcn)?;
            let method = class.find_method(method_name)?;
            return Some((class, method));
        }
        None
    }
}


#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub location: Option<SourceLocation>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub type_name: Option<Type>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    pub name: String,
    pub namespace: Option<String>,
    pub methods: Vec<Method>,
    pub location: Option<SourceLocation>,
}

#[allow(dead_code)]
impl Class {
    pub fn fully_qualified_name(&self) -> String {
        match &self.namespace {
            Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, self.name),
            _ => self.name.clone(),
        }
    }

    pub fn find_method(&self, name: &str) -> Option<&Method> {
        self.methods.iter().find(|m| m.name.eq_ignore_ascii_case(name))
    }

}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
    pub location: Option<SourceLocation>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    pub name: String,
    pub nullable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_class_name_with_import() {
        let mut file = FileModel::new(
            PathBuf::from("routes/web.php"),
            PathBuf::from("routes/web.php"),
        );
        file.imports.push(Import {
            path: "App\\Http\\Controllers\\UserController".to_string(),
            alias: None,
        });
        file.imports.push(Import {
            path: "App\\Services\\PaymentService".to_string(),
            alias: Some("Payment".to_string()),
        });

        assert_eq!(
            file.resolve_class_name("UserController"),
            "App\\Http\\Controllers\\UserController"
        );
        assert_eq!(
            file.resolve_class_name("Payment"),
            "App\\Services\\PaymentService"
        );
    }

    #[test]
    fn test_resolve_class_name_with_sub_namespace_and_alias() {
        let mut file = FileModel::new(
            PathBuf::from("routes/web.php"),
            PathBuf::from("routes/web.php"),
        );
        file.imports.push(Import {
            path: "App\\Http\\Controllers\\Auth".to_string(),
            alias: None,
        });
        file.imports.push(Import {
            path: "App\\Http\\Controllers\\Admin".to_string(),
            alias: Some("AdminPanel".to_string()),
        });

        assert_eq!(
            file.resolve_class_name("Auth\\LoginController"),
            "App\\Http\\Controllers\\Auth\\LoginController"
        );
        assert_eq!(
            file.resolve_class_name("AdminPanel\\UserController"),
            "App\\Http\\Controllers\\Admin\\UserController"
        );
        assert_eq!(
            file.resolve_class_name("App\\Http\\Controllers\\CustomController"),
            "App\\Http\\Controllers\\CustomController"
        );
    }

    #[test]
    fn test_resolve_class_name_with_namespace() {
        let mut file = FileModel::new(
            PathBuf::from("app/Http/Controllers/UserController.php"),
            PathBuf::from("app/Http/Controllers/UserController.php"),
        );
        file.namespace = Some("App\\Http\\Controllers".to_string());

        assert_eq!(
            file.resolve_class_name("OrderController"),
            "App\\Http\\Controllers\\OrderController"
        );
        assert_eq!(
            file.resolve_class_name("\\RootController"),
            "RootController"
        );
    }

    #[test]
    fn test_extract_path_parameters() {
        assert_eq!(extract_path_parameters("/users"), Vec::<String>::new());
        assert_eq!(extract_path_parameters("/users/{id}"), vec!["id"]);
        assert_eq!(
            extract_path_parameters("/posts/{post_id}/comments/{id?}"),
            vec!["post_id", "id"]
        );
        assert_eq!(
            extract_path_parameters("/posts/{post:slug}/comments/{comment:uuid?}"),
            vec!["post", "comment"]
        );
    }
}