use crate::completion::{
    ValueType, build_scope, find_active_workflow, helper_output_type, prompt_signature,
    type_map, value_type_from_type_expr,
};
use crate::source::{
    WorkspaceDocument, canonicalize_best_effort, load_import_elements_best_effort,
    load_workspace_documents_best_effort, parse_source,
};
use crate::symbols::{SymbolTargetKind, symbol_at_offset};
use auwgent_ast::{Element, Model, TypeDeclaration};
use auwgent_errors::Span;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverInfo {
    pub span: Span,
    pub contents: String,
}

pub fn hover_for_source(file: &Path, source: &str, offset: usize) -> Option<HoverInfo> {
    let root_path = canonicalize_best_effort(file);
    let parsed = parse_source(source);
    let symbol = symbol_at_offset(&parsed.model, offset)?;
    let merged_model = merged_model(&root_path, &parsed.model);
    let docs = load_workspace_documents_best_effort(&root_path, source);
    let visible_model = visible_model(&docs, &root_path);
    let visible_types = type_map(&visible_model);
    let active_workflow = find_active_workflow(&parsed.model, offset);

    let contents = match symbol.kind {
        SymbolTargetKind::Identifier(name) => {
            if let Some(workflow) = active_workflow {
                let scope = build_scope(&merged_model, workflow, offset);
                if let Some(ty) = scope.variables.get(&name) {
                    Some(format!("variable {}: {}", name, ty.format()))
                } else if let Some(signature) = scope.prompts.get(&name) {
                    Some(format!("prompt {}{}", name, prompt_suffix(signature)))
                } else {
                    None
                }
            } else {
                None
            }
        }
        SymbolTargetKind::Callable(name) => {
            if let Some(workflow) = active_workflow {
                let scope = build_scope(&merged_model, workflow, offset);
                if let Some(signature) = scope.tools.get(&name) {
                    Some(format!("tool {}{}", name, signature))
                } else if let Some(signature) = scope.prompts.get(&name) {
                    Some(format!("prompt {}{}", name, prompt_suffix(signature)))
                } else {
                    None
                }
            } else {
                None
            }
        }
        SymbolTargetKind::ContextField(name) => {
            let workflow = active_workflow?;
            let scope = build_scope(&merged_model, workflow, offset);
            scope
                .context_fields
                .get(&name)
                .map(|ty| format!("context {}: {}", name, ty.format()))
        }
        SymbolTargetKind::Helper(name) => hover_helper(&name, &docs, &root_path, &visible_types),
        SymbolTargetKind::Prompt(name) => hover_prompt(&name, &docs, &root_path, &visible_types),
        SymbolTargetKind::Type(name) => hover_type(&name, &docs, &root_path, &visible_types),
        SymbolTargetKind::Model(name) => hover_model(&name, &docs, &root_path),
        SymbolTargetKind::Member { root, path } => {
            let workflow = active_workflow?;
            let scope = build_scope(&merged_model, workflow, offset);
            resolve_member_hover(&scope, &root, &path)
        }
    }?;

    Some(HoverInfo {
        span: symbol.span,
        contents,
    })
}

fn merged_model(root_path: &Path, model: &Model) -> Model {
    let mut merged_elements = model.elements.clone();
    merged_elements.extend(load_import_elements_best_effort(root_path, &model.imports));
    Model {
        imports: model.imports.clone(),
        elements: merged_elements,
    }
}

fn visible_model(documents: &[WorkspaceDocument], root_path: &Path) -> Model {
    let mut elements = Vec::new();
    for document in documents {
        let is_root = document.path == root_path;
        elements.extend(
            document
                .model
                .elements
                .iter()
                .filter(|element| is_root || is_exported(element))
                .cloned(),
        );
    }

    Model {
        imports: Vec::new(),
        elements,
    }
}

fn resolve_member_hover(
    scope: &crate::completion::Scope,
    root: &str,
    path: &[String],
) -> Option<String> {
    let mut current = scope.variables.get(root)?.clone();
    for segment in path {
        current = current.member(segment)?.clone();
    }

    path.last()
        .map(|segment| format!("field {}: {}", segment, current.format()))
}

fn hover_helper(
    name: &str,
    documents: &[WorkspaceDocument],
    root_path: &Path,
    visible_types: &std::collections::HashMap<String, TypeDeclaration>,
) -> Option<String> {
    for document in documents {
        let is_root = document.path == root_path;
        for element in &document.model.elements {
            if let Element::Helper(helper) = element {
                if helper.name.value == name && (is_root || helper.exported) {
                    let output = helper_output_type(helper, visible_types).format();
                    return Some(format!("helper {} -> {}", name, output));
                }
            }
        }
    }

    None
}

fn hover_prompt(
    name: &str,
    documents: &[WorkspaceDocument],
    root_path: &Path,
    visible_types: &std::collections::HashMap<String, TypeDeclaration>,
) -> Option<String> {
    for document in documents {
        let is_root = document.path == root_path;
        for element in &document.model.elements {
            if let Element::NamedPrompt(prompt) = element {
                if prompt.name.value == name && (is_root || prompt.exported) {
                    let signature = prompt_signature(prompt, visible_types);
                    return Some(format!("prompt {}{}", name, prompt_suffix(&signature)));
                }
            }
        }
    }

    None
}

fn hover_type(
    name: &str,
    documents: &[WorkspaceDocument],
    root_path: &Path,
    visible_types: &std::collections::HashMap<String, TypeDeclaration>,
) -> Option<String> {
    for document in documents {
        let is_root = document.path == root_path;
        for element in &document.model.elements {
            if let Element::TypeDecl(declaration) = element {
                if declaration.name.value == name && (is_root || declaration.exported) {
                    let rendered = declaration_value_type(declaration, visible_types).format();
                    return Some(format!("type {} {}", name, rendered));
                }
            }
        }
    }

    None
}

fn hover_model(name: &str, documents: &[WorkspaceDocument], root_path: &Path) -> Option<String> {
    for document in documents {
        let is_root = document.path == root_path;
        for element in &document.model.elements {
            if let Element::ModelDef(model) = element {
                if model.name.value == name && (is_root || model.exported) {
                    return Some(format!("model {}", name));
                }
            }
        }
    }

    None
}

fn declaration_value_type(
    declaration: &TypeDeclaration,
    visible_types: &std::collections::HashMap<String, TypeDeclaration>,
) -> ValueType {
    let mut fields = std::collections::BTreeMap::new();
    for field in &declaration.fields {
        fields.insert(
            field.name.value.clone(),
            value_type_from_type_expr(&field.ty, visible_types),
        );
    }
    ValueType::Object(fields)
}

fn prompt_suffix(signature: &str) -> &str {
    signature.strip_prefix("prompt").unwrap_or(signature)
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
    use super::hover_for_source;

    #[test]
    fn hovers_variable_with_inferred_type() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_hover_variable_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let file = base.join("main.agent");
        let source = r#"
agent Demo {
    tool lookup(id: string): { id: string }

    workflow run(id: string): string {
        description: "run"
        let result = lookup(id)
        return result.id
    }
}
"#;
        std::fs::write(&file, source).unwrap();

        let offset = source.find("result.id").unwrap() + 1;
        let hover = hover_for_source(&file, source, offset).unwrap();

        assert_eq!(hover.contents, "variable result: { id: string }");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn hovers_imported_prompt_signature() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_hover_prompt_{}",
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
        let hover = hover_for_source(&file, source, offset).unwrap();

        assert_eq!(hover.contents, "prompt SharedPrompt(subject: string)");

        let _ = std::fs::remove_dir_all(&base);
    }
}