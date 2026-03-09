//! # auwgent-cli
//!
//! CLI binary for the Auwgent compiler.
//! Commands: compile, generate.

use clap::{Parser, Subcommand};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "auwgent", about = "Auwgent DSL Compiler", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a .agent file to IR JSON
    Compile {
        /// Path to the .agent file
        file: PathBuf,

        /// Output directory (defaults to same directory as input)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate type stubs from a .agent file
    Generate {
        /// Path to the .agent file
        file: PathBuf,

        /// Target language: ts or python
        #[arg(short, long, default_value = "ts")]
        target: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile { file, output } => compile(&file, output.as_deref()),
        Commands::Generate {
            file,
            target,
            output,
        } => generate(&file, &target, output.as_deref()),
    }
}

fn compile(file: &PathBuf, output: Option<&Path>) {
    let model = match load_model_with_imports(file) {
        Ok(model) => model,
        Err(()) => std::process::exit(1),
    };

    let filename = file.display().to_string();
    let source = std::fs::read_to_string(file).unwrap_or_default();

    let diagnostics = auwgent_checker::check(&model);
    if auwgent_errors::render_diagnostics(&diagnostics, &filename, &source) {
        std::process::exit(1);
    }

    let ir = match auwgent_ir::lower(&model) {
        Ok(ir) => ir,
        Err(errs) => {
            for error in &errs {
                eprintln!("Error: {}", error);
            }
            std::process::exit(1);
        }
    };

    let out_dir = output.unwrap_or_else(|| file.parent().unwrap());
    let stem = file.file_stem().unwrap().to_string_lossy();
    let out_path = out_dir.join(format!("{}.agent.json", stem));

    let json = serde_json::to_string_pretty(&ir).unwrap();
    std::fs::write(&out_path, json).unwrap();

    eprintln!("\x1b[Compiled → {}\x1b[0m", out_path.display());
}

fn generate(file: &PathBuf, target: &str, output: Option<&Path>) {
    let model = match load_model_with_imports(file) {
        Ok(model) => model,
        Err(()) => std::process::exit(1),
    };

    let ir = auwgent_ir::lower(&model).unwrap();

    let code = match target {
        "ts" | "typescript" => auwgent_codegen::generate_typescript(&ir),
        "py" | "python" => auwgent_codegen::generate_python(&ir),
        _ => {
            eprintln!("Unknown target '{}'. Use 'ts' or 'python'.", target);
            std::process::exit(1);
        }
    };

    let out_dir = output.unwrap_or_else(|| file.parent().unwrap());
    let stem = file.file_stem().unwrap().to_string_lossy();
    let ext = if target.starts_with("ts") {
        "types.ts"
    } else {
        "_types.py"
    };
    let out_path = out_dir.join(format!("{}.agent.{}", stem, ext));

    std::fs::write(&out_path, code).unwrap();
    eprintln!("\x1b[Generated → {}\x1b[0m", out_path.display());
}

fn load_model_with_imports(file: &Path) -> Result<auwgent_ast::Model, ()> {
    let canonical = match std::fs::canonicalize(file) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Error: could not resolve '{}': {}", file.display(), error);
            return Err(());
        }
    };

    let mut visited = HashSet::new();
    load_model_recursive(&canonical, &mut visited)
}

fn load_model_recursive(
    file: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<auwgent_ast::Model, ()> {
    if visited.contains(file) {
        return Ok(auwgent_ast::Model {
            imports: vec![],
            elements: vec![],
        });
    }
    visited.insert(file.to_path_buf());

    let filename = file.display().to_string();
    let source = match std::fs::read_to_string(file) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("Error: could not read '{}': {}", filename, error);
            return Err(());
        }
    };

    let (tokens, lex_errors) = auwgent_lexer::tokenize(&source);
    if !lex_errors.is_empty() {
        auwgent_errors::render_lex_errors(&lex_errors, &filename, &source);
        return Err(());
    }

    let (model, parse_errors) = auwgent_parser::parse(&tokens);
    if !parse_errors.is_empty() {
        auwgent_errors::render_diagnostics(&parse_errors, &filename, &source);
        return Err(());
    }

    let mut merged_elements = model.elements.clone();
    for import in &model.imports {
        let import_path = match resolve_import_path(file, &import.path.value) {
            Ok(path) => path,
            Err(message) => {
                eprintln!("Error: {}", message);
                return Err(());
            }
        };

        let imported_model = load_model_recursive(&import_path, visited)?;
        merged_elements.extend(select_imported_elements(&import.kind, &imported_model));
    }

    Ok(auwgent_ast::Model {
        imports: model.imports,
        elements: merged_elements,
    })
}

fn resolve_import_path(current_file: &Path, import_path: &str) -> Result<PathBuf, String> {
    let current_dir = current_file.parent().ok_or_else(|| {
        format!(
            "Could not resolve parent directory for '{}'",
            current_file.display()
        )
    })?;

    let with_extension = if import_path.ends_with(".agent") {
        import_path.to_string()
    } else {
        format!("{}.agent", import_path)
    };

    let candidate = current_dir.join(with_extension);
    if !candidate.exists() {
        return Err(format!(
            "Could not resolve import '{}' from '{}'",
            import_path,
            current_file.display()
        ));
    }

    std::fs::canonicalize(&candidate).map_err(|error| {
        format!(
            "Could not resolve import '{}' from '{}': {}",
            import_path,
            current_file.display(),
            error
        )
    })
}

fn select_imported_elements(
    import_shape: &auwgent_ast::ImportShape,
    model: &auwgent_ast::Model,
) -> Vec<auwgent_ast::Element> {
    match import_shape {
        auwgent_ast::ImportShape::Named(specifiers) => model
            .elements
            .iter()
            .filter(|element| is_named_import_match(specifiers, element))
            .cloned()
            .collect(),
        auwgent_ast::ImportShape::Wildcard { .. } => model
            .elements
            .iter()
            .filter(|element| is_exported_element(element))
            .cloned()
            .collect(),
    }
}

fn is_named_import_match(
    specifiers: &[auwgent_ast::ImportSpecifier],
    element: &auwgent_ast::Element,
) -> bool {
    let Some((name, exported)) = exported_element_name(element) else {
        return false;
    };

    exported
        && specifiers
            .iter()
            .any(|specifier| specifier.name.value == name)
}

fn is_exported_element(element: &auwgent_ast::Element) -> bool {
    exported_element_name(element)
        .map(|(_, exported)| exported)
        .unwrap_or(false)
}

fn exported_element_name(element: &auwgent_ast::Element) -> Option<(String, bool)> {
    match element {
        auwgent_ast::Element::Helper(helper) => Some((helper.name.value.clone(), helper.exported)),
        auwgent_ast::Element::TypeDecl(ty) => Some((ty.name.value.clone(), ty.exported)),
        auwgent_ast::Element::NamedPrompt(prompt) => {
            Some((prompt.name.value.clone(), prompt.exported))
        }
        auwgent_ast::Element::ModelDef(model) => Some((model.name.value.clone(), model.exported)),
        auwgent_ast::Element::Agent(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::load_model_with_imports;
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
}
