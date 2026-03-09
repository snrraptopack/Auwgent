use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Type {
    Const(String),
    Array(Box<Type>),
    Record {
        fields: HashMap<String, Type>,
        optional: HashMap<String, bool>,
    },
    Union(Vec<Type>),
    Error(String),
}

impl Type {
    pub(crate) fn string() -> Self {
        Type::Const("string".into())
    }

    pub(crate) fn number() -> Self {
        Type::Const("number".into())
    }

    pub(crate) fn boolean() -> Self {
        Type::Const("boolean".into())
    }

    pub(crate) fn error(msg: &str) -> Self {
        Type::Error(msg.into())
    }

    pub(crate) fn format(&self) -> String {
        match self {
            Type::Const(name) => name.clone(),
            Type::Array(element) => format!("{}[]", element.format()),
            Type::Record { fields, optional } => {
                let fields: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| {
                        let optional_suffix = if *optional.get(name).unwrap_or(&false) {
                            "?"
                        } else {
                            ""
                        };
                        format!("{}{}: {}", name, optional_suffix, value.format())
                    })
                    .collect();

                if fields.is_empty() {
                    "{}".into()
                } else {
                    format!("{{ {} }}", fields.join(", "))
                }
            }
            Type::Union(options) => options
                .iter()
                .map(|value| value.format())
                .collect::<Vec<_>>()
                .join(" | "),
            Type::Error(message) => format!("error({})", message),
        }
    }
}

pub(crate) struct TypeEnv {
    vars: HashMap<String, Type>,
}

impl TypeEnv {
    pub(crate) fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    pub(crate) fn set(&mut self, name: &str, ty: Type) {
        self.vars.insert(name.to_string(), ty);
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Type> {
        self.vars.get(name)
    }

    pub(crate) fn extend(&self) -> TypeEnv {
        TypeEnv {
            vars: self.vars.clone(),
        }
    }
}