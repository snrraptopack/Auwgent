//! Workspace and import graph analysis for the Auwgent compiler toolchain.
//!
//! This crate owns file loading, import resolution, and export-aware model
//! merging so the CLI, future LSP, and tests can share one source of truth.

use auwgent_ast::{Element, ImportShape, ImportSpecifier, Model};
use auwgent_errors::{Diagnostic, Span};
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum AnalysisError {
    CanonicalizeRoot {
        path: PathBuf,
        message: String,
    },
    ReadFile {
        path: PathBuf,
        message: String,
    },
    ResolveImport {
        current_file: PathBuf,
        import_path: String,
        message: String,
        span: Option<Span>,
    },
    Lex {
        path: PathBuf,
        source: String,
        diagnostics: Vec<Diagnostic>,
    },
    Parse {
        path: PathBuf,
        source: String,
        diagnostics: Vec<Diagnostic>,
    },
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisError::CanonicalizeRoot { path, message } => {
                write!(f, "could not resolve '{}': {}", path.display(), message)
            }
            AnalysisError::ReadFile { path, message } => {
                write!(f, "could not read '{}': {}", path.display(), message)
            }
            AnalysisError::ResolveImport {
                current_file,
                import_path,
                message,
                ..
            } => write!(
                f,
                "could not resolve import '{}' from '{}': {}",
                import_path,
                current_file.display(),
                message
            ),
            AnalysisError::Lex { path, .. } => {
                write!(f, "lexing failed for '{}'", path.display())
            }
            AnalysisError::Parse { path, .. } => {
                write!(f, "parsing failed for '{}'", path.display())
            }
        }
    }
}

impl std::error::Error for AnalysisError {}

pub fn load_model_with_imports(file: &Path) -> Result<Model, AnalysisError> {
    let canonical = std::fs::canonicalize(file).map_err(|error| AnalysisError::CanonicalizeRoot {
        path: file.to_path_buf(),
        message: error.to_string(),
    })?;

    let mut visited = HashSet::new();
    load_model_recursive(&canonical, &mut visited, None)
}

pub fn load_model_from_source_with_imports(
    file: &Path,
    source: &str,
) -> Result<Model, AnalysisError> {
    let canonical = std::fs::canonicalize(file).map_err(|error| AnalysisError::CanonicalizeRoot {
        path: file.to_path_buf(),
        message: error.to_string(),
    })?;

    let mut visited = HashSet::new();
    load_model_recursive(&canonical, &mut visited, Some(source))
}

pub fn resolve_import_path(
    current_file: &Path,
    import_path: &str,
) -> Result<PathBuf, AnalysisError> {
    resolve_import_path_with_span(current_file, import_path, None)
}

pub fn resolve_import_path_with_span(
    current_file: &Path,
    import_path: &str,
    span: Option<Span>,
) -> Result<PathBuf, AnalysisError> {
    let current_dir = current_file.parent().ok_or_else(|| AnalysisError::ResolveImport {
        current_file: current_file.to_path_buf(),
        import_path: import_path.to_string(),
        message: "missing parent directory".to_string(),
        span,
    })?;

    let with_extension = if import_path.ends_with(".agent") {
        import_path.to_string()
    } else {
        format!("{}.agent", import_path)
    };

    let candidate = current_dir.join(with_extension);
    if !candidate.exists() {
        return Err(AnalysisError::ResolveImport {
            current_file: current_file.to_path_buf(),
            import_path: import_path.to_string(),
            message: "file does not exist".to_string(),
            span,
        });
    }

    std::fs::canonicalize(&candidate).map_err(|error| AnalysisError::ResolveImport {
        current_file: current_file.to_path_buf(),
        import_path: import_path.to_string(),
        message: error.to_string(),
        span,
    })
}

fn load_model_recursive(
    file: &Path,
    visited: &mut HashSet<PathBuf>,
    root_source: Option<&str>,
) -> Result<Model, AnalysisError> {
    if visited.contains(file) {
        return Ok(Model {
            imports: vec![],
            elements: vec![],
        });
    }
    visited.insert(file.to_path_buf());

    let source = match root_source {
        Some(source) => source.to_string(),
        None => std::fs::read_to_string(file).map_err(|error| AnalysisError::ReadFile {
            path: file.to_path_buf(),
            message: error.to_string(),
        })?,
    };

    let (tokens, lex_errors) = auwgent_lexer::tokenize(&source);
    if !lex_errors.is_empty() {
        return Err(AnalysisError::Lex {
            path: file.to_path_buf(),
            source,
            diagnostics: lex_errors,
        });
    }

    let (model, parse_errors) = auwgent_parser::parse(&tokens);
    if !parse_errors.is_empty() {
        return Err(AnalysisError::Parse {
            path: file.to_path_buf(),
            source,
            diagnostics: parse_errors,
        });
    }

    let mut merged_elements = model.elements.clone();
    for import in &model.imports {
        let import_path = resolve_import_path_with_span(file, &import.path.value, Some(import.path.span))?;
        let imported_model = load_model_recursive(&import_path, visited, None)?;
        merged_elements.extend(select_imported_elements(&import.kind, &imported_model));
    }

    Ok(Model {
        imports: model.imports,
        elements: merged_elements,
    })
}

fn select_imported_elements(import_shape: &ImportShape, model: &Model) -> Vec<Element> {
    match import_shape {
        ImportShape::Named(specifiers) => model
            .elements
            .iter()
            .filter(|element| is_named_import_match(specifiers, element))
            .cloned()
            .collect(),
        ImportShape::Wildcard { .. } => model
            .elements
            .iter()
            .filter(|element| is_exported_element(element))
            .cloned()
            .collect(),
    }
}

fn is_named_import_match(specifiers: &[ImportSpecifier], element: &Element) -> bool {
    let Some((name, exported)) = exported_element_name(element) else {
        return false;
    };

    exported && specifiers.iter().any(|specifier| specifier.name.value == name)
}

fn is_exported_element(element: &Element) -> bool {
    exported_element_name(element)
        .map(|(_, exported)| exported)
        .unwrap_or(false)
}

fn exported_element_name(element: &Element) -> Option<(String, bool)> {
    match element {
        Element::Helper(helper) => Some((helper.name.value.clone(), helper.exported)),
        Element::TypeDecl(ty) => Some((ty.name.value.clone(), ty.exported)),
        Element::NamedPrompt(prompt) => Some((prompt.name.value.clone(), prompt.exported)),
        Element::ModelDef(model) => Some((model.name.value.clone(), model.exported)),
        Element::Agent(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{load_model_from_source_with_imports, load_model_with_imports};
    use serde_json::json;

    #[test]
    fn imported_prompt_is_lowered_as_prompt_ref() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_import_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let prompt_file = base.join("prompt.agent");
        let main_file = base.join("main.agent");

        std::fs::write(
            &prompt_file,
            r#"
export prompt MainAgentPrompt{
    """
        You are the Main Agent in a multi-agent system.
        {{@schema(output)}}
    """
}
"#,
        )
        .unwrap();

        std::fs::write(
            &main_file,
            r#"
import {MainAgentPrompt} from "./prompt"

prompt One{
   example{
    user: "hello"
    assistant: "how may i help you"

    user: "can you help me with something"
    assistant: "sure what is it"
   }

   MainAgentPrompt
}

agent Test{
    default config{
        model:gemini("gemini-2.5-flash",{
            thinking:"low",
            maxToken:2000
        })
        prompt:"Hello" + One
    }

    input{
        text:string
    }

    output{
        name:string
        age:string
    }
}
"#,
        )
        .unwrap();

        let model = load_model_with_imports(&main_file).unwrap();
        let ir = auwgent_ir::lower(&model).unwrap();

        assert_eq!(
            ir["modelConfig"][0]["defaultConfig"]["prompt"]["right"]["value"][1]["type"],
            json!("promptRef")
        );
        assert_eq!(
            ir["modelConfig"][0]["defaultConfig"]["prompt"]["right"]["value"][1]["name"],
            json!("MainAgentPrompt")
        );
        assert_eq!(
            ir["modelConfig"][0]["defaultConfig"]["prompt"]["right"]["value"][1]["value"][0]["type"],
            json!("template")
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn root_source_override_is_used_for_import_analysis() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_analysis_root_override_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let shared_file = base.join("shared.agent");
        let main_file = base.join("main.agent");

        std::fs::write(
            &shared_file,
            r#"
export prompt ImportedPrompt {
    "hello"
}
"#,
        )
        .unwrap();

        std::fs::write(&main_file, "agent Placeholder {}\n").unwrap();

        let model = load_model_from_source_with_imports(
            &main_file,
            r#"
import {ImportedPrompt} from "./shared"

agent Demo {
    default config {
        model: gemini("gemini-2.5-flash")
        prompt: ImportedPrompt
    }
}
"#,
        )
        .unwrap();

        let ir = auwgent_ir::lower(&model).unwrap();
        assert_eq!(
            ir["modelConfig"][0]["defaultConfig"]["prompt"]["type"],
            json!("promptRef")
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}