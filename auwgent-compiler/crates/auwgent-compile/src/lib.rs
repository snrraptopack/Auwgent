use auwgent_analysis::{load_model_from_source_with_imports, load_model_with_imports, AnalysisError};
use auwgent_ast::Model;
use auwgent_errors::{Diagnostic, Severity};
use auwgent_ir_schema::AgentIR;
use std::path::Path;

/// The result of running the shared compile validation pipeline.
///
/// `model` is always returned when parsing/import resolution succeeded, even if
/// validation later produced diagnostics. `ir` is only present when lowering
/// succeeded.
#[derive(Debug, Clone)]
pub struct CompileValidation {
    pub model: Model,
    pub diagnostics: Vec<Diagnostic>,
    pub ir: Option<AgentIR>,
}

/// Run the same compile-oriented validation pipeline on an in-memory source
/// buffer that the CLI and LSP can both share.
///
/// Pipeline:
/// 1. Parse + import resolution
/// 2. Semantic checker
/// 3. IR lowering
///
/// This mirrors the CLI's notion of "compile validity" much more closely than
/// checker-only validation.
pub fn validate_source_for_compile(
    file: &Path,
    source: &str,
) -> Result<CompileValidation, AnalysisError> {
    let model = load_model_from_source_with_imports(file, source)?;
    Ok(validate_loaded_model(model))
}

/// Run the shared compile-oriented validation pipeline for a file on disk.
pub fn validate_file_for_compile(file: &Path) -> Result<CompileValidation, AnalysisError> {
    let model = load_model_with_imports(file)?;
    Ok(validate_loaded_model(model))
}

fn validate_loaded_model(model: Model) -> CompileValidation {
    let mut diagnostics = auwgent_checker::check(&model);

    match auwgent_ir::lower(&model) {
        Ok(ir) => CompileValidation {
            model,
            diagnostics,
            ir: Some(ir),
        },
        Err(lowering_diagnostics) => {
            diagnostics.extend(
                lowering_diagnostics
                    .into_iter()
                    .filter(|diagnostic| {
                        !(diagnostic.severity == Severity::Info
                            && diagnostic.message.trim() == "no agent found in file")
                    }),
            );

            CompileValidation {
                model,
                diagnostics,
                ir: None,
            }
        }
    }
}

/// Returns true when the diagnostic list contains at least one error.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}