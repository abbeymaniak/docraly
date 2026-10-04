#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
}

#[derive(Debug)]
pub struct Parameter {
    pub name: String,
    pub type_name: Option<Type>,
}

#[derive(Debug)]
pub struct Class {
    pub name: String,
    pub namespace: Option<String>,
    pub methods: Vec<Method>,
}

#[derive(Debug)]
pub struct Method {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<Type>,
}

#[derive(Debug)]
pub struct Type {
    pub name: String,
    pub nullable: bool,
}