use crate::completion::find_active_workflow;
use crate::source::{
    WorkspaceDocument, canonicalize_best_effort, load_workspace_documents_best_effort,
    parse_source,
};
use crate::symbols::{
    SymbolTargetKind, find_context_field_definition, find_local_variable_definition,
    find_tool_definition, symbol_at_offset,
};
use auwgent_ast::Element;
use auwgent_errors::Span;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionTarget {
    pub path: PathBuf,
    pub source: String,
    pub span: Span,
}

pub fn definition_for_source(file: &Path, source: &str, offset: usize) -> Option<DefinitionTarget> {
    let root_path = canonicalize_best_effort(file);
    let parsed = parse_source(source);
    let symbol = symbol_at_offset(&parsed.model, offset)?;
    let documents = load_workspace_documents_best_effort(&root_path, source);
    let active_workflow = find_active_workflow(&parsed.model, offset);

    match symbol.kind {
        SymbolTargetKind::Identifier(name) => {
            if let Some(workflow) = active_workflow {
                if let Some(span) = find_local_variable_definition(&workflow, offset, &name) {
                    return Some(DefinitionTarget {
                        path: root_path.clone(),
                        source: source.to_string(),
                        span,
                    });
                }
            }

            find_named_top_level_definition(&documents, &root_path, &name, matches_prompt)
        }
        SymbolTargetKind::Callable(name) => {
            if let Some(workflow) = active_workflow {
                if let Some(span) = find_tool_definition(&workflow, &name) {
                    return Some(DefinitionTarget {
                        path: root_path.clone(),
                        source: source.to_string(),
                        span,
                    });
                }
            }

            find_named_top_level_definition(&documents, &root_path, &name, matches_prompt)
        }
        SymbolTargetKind::ContextField(name) => {
            let workflow = active_workflow?;
            let span = find_context_field_definition(&workflow, &name)?;
            Some(DefinitionTarget {
                path: root_path,
                source: source.to_string(),
                span,
            })
        }
        SymbolTargetKind::Helper(name) => {
            find_named_top_level_definition(&documents, &root_path, &name, matches_helper)
        }
        SymbolTargetKind::Prompt(name) => {
            find_named_top_level_definition(&documents, &root_path, &name, matches_prompt)
        }
        SymbolTargetKind::Type(name) => {
            find_named_top_level_definition(&documents, &root_path, &name, matches_type)
        }
        SymbolTargetKind::Model(name) => {
            find_named_top_level_definition(&documents, &root_path, &name, matches_model)
        }
        SymbolTargetKind::Member { root, .. } => {
            if let Some(workflow) = active_workflow {
                if let Some(span) = find_local_variable_definition(&workflow, offset, &root) {
                    return Some(DefinitionTarget {
                        path: root_path,
                        source: source.to_string(),
                        span,
                    });
                }
            }
            None
        }
    }
}

fn find_named_top_level_definition(
    documents: &[WorkspaceDocument],
    root_path: &Path,
    name: &str,
    matcher: fn(&Element, &str) -> Option<Span>,
) -> Option<DefinitionTarget> {
    for document in documents {
        let is_root = document.path == root_path;
        for element in &document.model.elements {
            if !is_root && !is_exported(element) {
                continue;
            }

            if let Some(span) = matcher(element, name) {
                return Some(DefinitionTarget {
                    path: document.path.clone(),
                    source: document.source.clone(),
                    span,
                });
            }
        }
    }

    None
}

fn matches_helper(element: &Element, name: &str) -> Option<Span> {
    match element {
        Element::Helper(helper) if helper.name.value == name => Some(helper.name.span),
        _ => None,
    }
}

fn matches_prompt(element: &Element, name: &str) -> Option<Span> {
    match element {
        Element::NamedPrompt(prompt) if prompt.name.value == name => Some(prompt.name.span),
        _ => None,
    }
}

fn matches_type(element: &Element, name: &str) -> Option<Span> {
    match element {
        Element::TypeDecl(declaration) if declaration.name.value == name => Some(declaration.name.span),
        _ => None,
    }
}

fn matches_model(element: &Element, name: &str) -> Option<Span> {
    match element {
        Element::ModelDef(model) if model.name.value == name => Some(model.name.span),
        _ => None,
    }
}

fn is_exported(element: &Element) -> bool {
    match element {
        Element::Helper(helper) => helper.exported,
        Element::ComponentDecl(component) => component.exported,
        Element::TypeDecl(declaration) => declaration.exported,
        Element::NamedPrompt(prompt) => prompt.exported,
        Element::ModelDef(model) => model.exported,
        Element::IntentDecl(intent) => intent.exported,
        Element::Agent(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::definition_for_source;

    #[test]
    fn resolves_local_variable_definition() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_definition_variable_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let file = base.join("main.agent");
        let source = r#"
agent Demo {
    workflow run(): string {
        description: "run"
        let result = "ok"
        return result
    }
}
"#;
        std::fs::write(&file, source).unwrap();

        let offset = source.rfind("result").unwrap() + 1;
        let target = definition_for_source(&file, source, offset).unwrap();

        assert_eq!(target.path, std::fs::canonicalize(&file).unwrap());
        assert_eq!(&target.source[target.span.start..target.span.end], "result");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn resolves_imported_prompt_definition() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_definition_prompt_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let shared = base.join("shared.agent");
        std::fs::write(
            &shared,
            r#"
export prompt SharedPrompt(subject: string) {
    subject
}
"#,
        )
        .unwrap();

        let file = base.join("main.agent");
        let source = r#"
import { SharedPrompt } from "./shared"

agent Demo {
    workflow run(subject: string): string {
        description: "run"
        return SharedPrompt(subject)
    }
}
"#;
        std::fs::write(&file, source).unwrap();

        let offset = source.rfind("SharedPrompt").unwrap() + 1;
        let target = definition_for_source(&file, source, offset).unwrap();

        assert_eq!(target.path, std::fs::canonicalize(&shared).unwrap());
        assert_eq!(&target.source[target.span.start..target.span.end], "SharedPrompt");

        let _ = std::fs::remove_dir_all(&base);
    }
}
