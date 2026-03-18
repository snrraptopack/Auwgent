use auwgent_analysis::AnalysisError;

pub fn report_analysis_error(error: &AnalysisError) {
    match error {
        AnalysisError::Lex {
            path,
            source,
            diagnostics,
        }
        | AnalysisError::Parse {
            path,
            source,
            diagnostics,
        } => {
            auwgent_errors::render_diagnostics(diagnostics, &path.display().to_string(), source);
        }
        AnalysisError::ResolveImport {
            current_file,
            import_path,
            message,
            ..
        } => {
            eprintln!(
                "Error: could not resolve import '{}' from '{}': {}",
                import_path,
                current_file.display(),
                message
            );
        }
        _ => eprintln!("Error: {}", error),
    }
}
