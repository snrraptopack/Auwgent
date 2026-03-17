//! Workspace and import graph analysis for the Auwgent compiler toolchain.
//!
//! This module owns file loading, import resolution, and export-aware model
//! merging so the CLI, future LSP, and tests can share one source of truth.

use auwgent_ast::{Element, FileImport, ImportShape, ImportSpecifier, Model};
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

#[derive(Debug, Clone)]
pub(crate) struct ParsedSource {
    pub model: Model,
    pub lex_diagnostics: Vec<Diagnostic>,
    pub parse_diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceDocument {
    pub path: PathBuf,
    pub source: String,
    pub model: Model,
}

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

pub fn best_effort_model_from_source_with_imports(
    file: &Path,
    source: &str,
) -> Result<(Model, Vec<Diagnostic>), AnalysisError> {
    let canonical = std::fs::canonicalize(file).map_err(|error| AnalysisError::CanonicalizeRoot {
        path: file.to_path_buf(),
        message: error.to_string(),
    })?;

    let parsed = parse_source(source);
    let mut diagnostics = parsed.lex_diagnostics.clone();
    diagnostics.extend(parsed.parse_diagnostics.clone());

    let mut merged_elements = parsed.model.elements.clone();
    merged_elements.extend(load_import_elements_best_effort(&canonical, &parsed.model.imports));

    let merged_model = Model {
        imports: parsed.model.imports,
        elements: merged_elements,
    };

    Ok((merged_model, diagnostics))
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

pub(crate) fn parse_source(source: &str) -> ParsedSource {
    let (tokens, lex_diagnostics) = auwgent_lexer::tokenize(source);
    let (model, parse_diagnostics) = auwgent_parser::parse(&tokens);

    ParsedSource {
        model,
        lex_diagnostics,
        parse_diagnostics,
    }
}

pub(crate) fn canonicalize_best_effort(file: &Path) -> PathBuf {
    std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf())
}

pub(crate) fn load_import_elements_best_effort(current_file: &Path, imports: &[FileImport]) -> Vec<Element> {
    let mut visited = HashSet::new();
    let mut merged = Vec::new();

    for import in imports {
        let Ok(path) = resolve_import_path_with_span(current_file, &import.path.value, Some(import.path.span)) else {
            continue;
        };
        merged.extend(load_import_recursive_best_effort(&path, &mut visited, &import.kind));
    }

    merged
}

pub(crate) fn load_workspace_documents_best_effort(
    file: &Path,
    root_source: &str,
) -> Vec<WorkspaceDocument> {
    let canonical = canonicalize_best_effort(file);
    let mut visited = HashSet::new();
    let mut documents = Vec::new();
    load_workspace_documents_recursive(
        &canonical,
        Some(root_source.to_string()),
        &mut visited,
        &mut documents,
    );
    documents
}

fn load_workspace_documents_recursive(
    file: &Path,
    root_source: Option<String>,
    visited: &mut HashSet<PathBuf>,
    documents: &mut Vec<WorkspaceDocument>,
) {
    if visited.contains(file) {
        return;
    }
    visited.insert(file.to_path_buf());

    let source = match root_source {
        Some(source) => source,
        None => {
            let Ok(source) = std::fs::read_to_string(file) else {
                return;
            };
            source
        }
    };

    let parsed = parse_source(&source);
    let model = parsed.model.clone();
    documents.push(WorkspaceDocument {
        path: file.to_path_buf(),
        source,
        model: model.clone(),
    });

    for import in &model.imports {
        let Ok(import_path) = resolve_import_path_with_span(file, &import.path.value, Some(import.path.span)) else {
            continue;
        };
        load_workspace_documents_recursive(&import_path, None, visited, documents);
    }
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

    let parsed = parse_source(&source);
    if !parsed.lex_diagnostics.is_empty() {
        return Err(AnalysisError::Lex {
            path: file.to_path_buf(),
            source,
            diagnostics: parsed.lex_diagnostics,
        });
    }

    if !parsed.parse_diagnostics.is_empty() {
        return Err(AnalysisError::Parse {
            path: file.to_path_buf(),
            source,
            diagnostics: parsed.parse_diagnostics,
        });
    }

    let mut merged_elements = parsed.model.elements.clone();
    for import in &parsed.model.imports {
        let import_path = resolve_import_path_with_span(file, &import.path.value, Some(import.path.span))?;
        let imported_model = load_model_recursive(&import_path, visited, None)?;
        merged_elements.extend(select_imported_elements(&import.kind, &imported_model));
    }

    Ok(Model {
        imports: parsed.model.imports,
        elements: merged_elements,
    })
}

fn load_import_recursive_best_effort(
    file: &Path,
    visited: &mut HashSet<PathBuf>,
    import_shape: &ImportShape,
) -> Vec<Element> {
    if visited.contains(file) {
        return Vec::new();
    }
    visited.insert(file.to_path_buf());


    let Ok(source) = std::fs::read_to_string(file) else {
        return Vec::new();
    };

    let parsed = parse_source(&source);
    let mut merged = select_imported_elements(import_shape, &parsed.model);
    for import in &parsed.model.imports {
        let Ok(import_path) = resolve_import_path_with_span(file, &import.path.value, Some(import.path.span)) else {
            continue;
        };
        merged.extend(load_import_recursive_best_effort(&import_path, visited, &import.kind));
    }
    merged
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
        Element::IntentDecl(intent) => Some((intent.name.value.clone(), intent.exported)),
        Element::Agent(_) => None,
    }
}