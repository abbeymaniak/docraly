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
        }
    }

    pub fn find_class(&self, name: &str) -> Option<&Class> {
        self.classes.iter().find(|c| c.name == name)
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

    pub fn find_class_by_fqcn(&self, fqcn: &str) -> Option<&Class> {
        self.all_classes().find(|c| c.fully_qualified_name() == fqcn)
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
        self.methods.iter().find(|m| m.name == name)
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