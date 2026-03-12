use crate::completion::find_active_workflow;
use crate::source::{
    WorkspaceDocument, canonicalize_best_effort, load_workspace_documents_best_effort,
    parse_source,
};
use crate::symbols::{
    SymbolTarget, SymbolTargetKind, find_context_field_definition, find_local_variable_definition,
    find_tool_definition, symbol_at_offset, symbol_occurrences, workflow_symbol_occurrences,
};
use auwgent_ast::Element;
use auwgent_errors::Span;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceTarget {
    pub path: PathBuf,
    pub source: String,
    pub span: Span,
}

pub fn references_for_source(file: &Path, source: &str, offset: usize) -> Vec<ReferenceTarget> {
    let root_path = canonicalize_best_effort(file);
    let parsed = parse_source(source);
    let Some(symbol) = symbol_at_offset(&parsed.model, offset) else {
        return Vec::new();
    };
    let documents = load_workspace_documents_best_effort(&root_path, source);
    let active_workflow = find_active_workflow(&parsed.model, offset);

    match &symbol.kind {
        SymbolTargetKind::Identifier(name) => {
            if let Some(workflow) = active_workflow {
                if find_local_variable_definition(&workflow, offset, name).is_some() {
                    return workflow_symbol_occurrences(&workflow)
                        .into_iter()
                        .filter(|candidate| matches!(candidate.kind, SymbolTargetKind::Identifier(ref candidate_name) if candidate_name == name))
                        .map(|candidate| root_reference(source, &root_path, candidate.span))
                        .collect();
                }
            }
            Vec::new()
        }
        SymbolTargetKind::Callable(name) => {
            if let Some(workflow) = active_workflow {
                if find_tool_definition(&workflow, name).is_some() {
                    return workflow_symbol_occurrences(&workflow)
                        .into_iter()
                        .filter(|candidate| matches!(candidate.kind, SymbolTargetKind::Callable(ref candidate_name) if candidate_name == name))
                        .map(|candidate| root_reference(source, &root_path, candidate.span))
                        .collect();
                }
            }
            collect_workspace_references(
                &documents,
                &root_path,
                &symbol,
                |candidate| {
                    matches!(&candidate.kind, SymbolTargetKind::Prompt(candidate_name) if candidate_name == name)
                        || matches!(&candidate.kind, SymbolTargetKind::Callable(candidate_name) if candidate_name == name)
                },
            )
        }
        SymbolTargetKind::ContextField(name) => {
            if let Some(workflow) = active_workflow {
                if let Some(span) = find_context_field_definition(&workflow, name) {
                    let mut refs = vec![root_reference(source, &root_path, span)];
                    refs.extend(
                        workflow_symbol_occurrences(&workflow)
                            .into_iter()
                            .filter(|candidate| matches!(candidate.kind, SymbolTargetKind::ContextField(ref candidate_name) if candidate_name == name))
                            .map(|candidate| root_reference(source, &root_path, candidate.span)),
                    );
                    refs.sort_by_key(|target| (target.span.start, target.span.end));
                    refs.dedup_by(|left, right| left.path == right.path && left.span == right.span);
                    return refs;
                }
            }
            Vec::new()
        }
        SymbolTargetKind::Helper(name) => collect_workspace_references(
            &documents,
            &root_path,
            &symbol,
            |candidate| matches!(&candidate.kind, SymbolTargetKind::Helper(candidate_name) if candidate_name == name),
        ),
        SymbolTargetKind::Prompt(name) => collect_workspace_references(
            &documents,
            &root_path,
            &symbol,
            |candidate| matches!(&candidate.kind, SymbolTargetKind::Prompt(candidate_name) if candidate_name == name),
        ),
        SymbolTargetKind::Type(name) => collect_workspace_references(
            &documents,
            &root_path,
            &symbol,
            |candidate| matches!(&candidate.kind, SymbolTargetKind::Type(candidate_name) if candidate_name == name),
        ),
        SymbolTargetKind::Model(name) => collect_workspace_references(
            &documents,
            &root_path,
            &symbol,
            |candidate| matches!(&candidate.kind, SymbolTargetKind::Model(candidate_name) if candidate_name == name),
        ),
        SymbolTargetKind::Member { root, path } => {
            if let Some(workflow) = active_workflow {
                return workflow_symbol_occurrences(&workflow)
                    .into_iter()
                    .filter(|candidate| {
                        matches!(&candidate.kind, SymbolTargetKind::Member { root: candidate_root, path: candidate_path } if candidate_root == root && candidate_path == path)
                    })
                    .map(|candidate| root_reference(source, &root_path, candidate.span))
                    .collect();
            }
            Vec::new()
        }
    }
}

fn collect_workspace_references(
    documents: &[WorkspaceDocument],
    root_path: &Path,
    source_symbol: &SymbolTarget,
    matcher: impl Fn(&SymbolTarget) -> bool,
) -> Vec<ReferenceTarget> {
    let mut refs = Vec::new();

    for document in documents {
        let is_root = document.path == root_path;
        for element in &document.model.elements {
            if !is_root && !is_exported(element) {
                continue;
            }
        }

        refs.extend(
            symbol_occurrences(&document.model)
                .into_iter()
                .filter(|candidate| matcher(candidate))
                .map(|candidate| ReferenceTarget {
                    path: document.path.clone(),
                    source: document.source.clone(),
                    span: candidate.span,
                }),
        );
    }

    refs.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.span.start.cmp(&right.span.start))
            .then(left.span.end.cmp(&right.span.end))
    });
    refs.dedup_by(|left, right| left.path == right.path && left.span == right.span);

    if refs.is_empty() {
        refs.push(ReferenceTarget {
            path: root_path.to_path_buf(),
            source: String::new(),
            span: source_symbol.span,
        });
    }

    refs
}

fn root_reference(source: &str, path: &Path, span: Span) -> ReferenceTarget {
    ReferenceTarget {
        path: path.to_path_buf(),
        source: source.to_string(),
        span,
    }
}

fn is_exported(element: &Element) -> bool {
    match element {
        Element::Helper(helper) => helper.exported,
        Element::TypeDecl(declaration) => declaration.exported,
        Element::NamedPrompt(prompt) => prompt.exported,
        Element::ModelDef(model) => model.exported,
        Element::Agent(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::references_for_source;

    #[test]
    fn finds_local_variable_references() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_references_variable_{}",
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
        result = result + "!"
        return result
    }
}
"#;
        std::fs::write(&file, source).unwrap();

        let offset = source.rfind("result").unwrap() + 1;
        let refs = references_for_source(&file, source, offset);
        let labels = refs
            .iter()
            .map(|target| target.source[target.span.start..target.span.end].to_string())
            .collect::<Vec<_>>();

        assert_eq!(refs.len(), 4, "labels: {:?}", labels);
        assert!(labels.iter().all(|label| label == "result"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn finds_imported_prompt_references() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_references_prompt_{}",
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
        let refs = references_for_source(&file, source, offset);

        assert_eq!(refs.len(), 2);
        assert_eq!(&refs[0].source[refs[0].span.start..refs[0].span.end], "SharedPrompt");
        assert_eq!(&refs[1].source[refs[1].span.start..refs[1].span.end], "SharedPrompt");

        let _ = std::fs::remove_dir_all(&base);
    }
}