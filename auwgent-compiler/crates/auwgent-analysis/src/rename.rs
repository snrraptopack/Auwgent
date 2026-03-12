use crate::reference::references_for_source;
use auwgent_errors::Span;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameEdit {
    pub path: PathBuf,
    pub source: String,
    pub span: Span,
    pub new_text: String,
}

pub fn rename_for_source(
    file: &Path,
    source: &str,
    offset: usize,
    new_name: &str,
) -> Vec<RenameEdit> {
    if !is_valid_identifier(new_name) {
        return Vec::new();
    }

    references_for_source(file, source, offset)
        .into_iter()
        .filter(|target| !target.source.is_empty())
        .map(|target| RenameEdit {
            path: target.path,
            source: target.source,
            span: target.span,
            new_text: new_name.to_string(),
        })
        .collect()
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }

    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::rename_for_source;

    #[test]
    fn renames_local_variable_occurrences() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_rename_variable_{}",
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
        let edits = rename_for_source(&file, source, offset, "finalResult");

        assert_eq!(edits.len(), 4);
        assert!(edits.iter().all(|edit| edit.new_text == "finalResult"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn renames_imported_prompt_occurrences() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_rename_prompt_{}",
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
        let edits = rename_for_source(&file, source, offset, "ProjectPrompt");

        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "ProjectPrompt");
        assert_eq!(edits[1].new_text, "ProjectPrompt");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_invalid_identifier_names() {
        let base = std::env::temp_dir().join(format!(
            "auwgent_rename_invalid_{}",
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
        let edits = rename_for_source(&file, source, offset, "123bad");

        assert!(edits.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }
}