//! Annotation kinds — every `@name` token the lexer can recognize.
//!
//! Unknown annotations produce `AnnotationKind::Unknown` rather than an error,
//! so the parser can emit a "unknown annotation" diagnostic with a good span.

/// The kind of an `@annotation` token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationKind {
    /// `@tool` — marks a function as a DSL-native callable tool.
    Tool,
    /// `@desc "..."` — description string shown to the model.
    Desc,
    /// `@middleware("name")` — declares a DSL middleware function.
    Middleware,
    /// `@middlewares(Name, ...)` — attaches middleware(s) to an agent.
    Middlewares,
    /// `@context(Type)` — binds a context type to an agent.
    Context,
    /// `@native` — forces native provider tool-calling mode on an agent.
    Native,
    /// `@block` — forces block protocol mode on an agent.
    Block,
    /// Any `@name` not matching the above — checker will emit a diagnostic.
    Unknown,
}

impl AnnotationKind {
    /// Map an `@annotation` slice (e.g. `"@tool"`) to its kind.
    pub fn from_slice(s: &str) -> Self {
        match s {
            "@tool" => Self::Tool,
            "@desc" => Self::Desc,
            "@middleware" => Self::Middleware,
            "@middlewares" => Self::Middlewares,
            "@context" => Self::Context,
            "@native" => Self::Native,
            "@block" => Self::Block,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_annotations_classify_correctly() {
        assert_eq!(AnnotationKind::from_slice("@tool"), AnnotationKind::Tool);
        assert_eq!(AnnotationKind::from_slice("@desc"), AnnotationKind::Desc);
        assert_eq!(AnnotationKind::from_slice("@middleware"), AnnotationKind::Middleware);
        assert_eq!(AnnotationKind::from_slice("@middlewares"), AnnotationKind::Middlewares);
        assert_eq!(AnnotationKind::from_slice("@context"), AnnotationKind::Context);
        assert_eq!(AnnotationKind::from_slice("@native"), AnnotationKind::Native);
        assert_eq!(AnnotationKind::from_slice("@block"), AnnotationKind::Block);
    }

    #[test]
    fn unknown_annotation_does_not_panic() {
        assert_eq!(AnnotationKind::from_slice("@toolbox"), AnnotationKind::Unknown);
        assert_eq!(AnnotationKind::from_slice("@anything"), AnnotationKind::Unknown);
        assert_eq!(AnnotationKind::from_slice("@"), AnnotationKind::Unknown);
    }

    #[test]
    fn annotation_kind_is_copy() {
        let a = AnnotationKind::Tool;
        let _b = a; // copy
        assert_eq!(a, AnnotationKind::Tool);
    }
}
