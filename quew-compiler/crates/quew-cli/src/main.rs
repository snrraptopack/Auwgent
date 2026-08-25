//! quew command-line compiler entry point.

use std::{fs, path::PathBuf, process::ExitCode, sync::Arc};

use clap::{Parser, Subcommand};
use quew_errors::{Diagnostic, Severity};
use quew_interner::Interner;
use quew_source::SourceMap;
use quew_stdlib as _;

#[derive(Parser)]
#[command(name = "quew", version, about = "Compile and check quew source files")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and type-check a .quew file.
    Check { file: PathBuf },
    /// Parse, type-check, lower to in-memory graph IR, and print a summary.
    Compile { file: PathBuf },

    /// Parse, compile, and run a .quew file.
    Run {
        file: PathBuf,
        /// The target function or agent to execute (e.g. "function:main")
        #[arg(long, default_value = "function:main")]
        target: String,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run() -> Result<(), ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { file } => {
            let pipeline = compile_frontend(&file)?;
            emit_diagnostics(&pipeline.diagnostics);
            if has_errors(&pipeline.diagnostics) {
                return Err(ExitCode::from(1));
            }
            println!("check ok: {} item(s)", pipeline.parse.module.items.len());
            Ok(())
        }
        Command::Compile { file } => {
            let pipeline = compile_frontend(&file)?;
            emit_diagnostics(&pipeline.diagnostics);
            if has_errors(&pipeline.diagnostics) {
                return Err(ExitCode::from(1));
            }

            let ir = quew_ir::lower::lower(&pipeline.module, &pipeline.check, &pipeline.interner);
            let node_count: usize = ir.graphs.values().map(|graph| graph.nodes.len()).sum();
            let edge_count: usize = ir.graphs.values().map(|graph| graph.edges.len()).sum();

            println!("compile ok");
            println!(
                "entry agent: {}",
                pipeline.interner.resolve(ir.program.entry_agent)
            );
            println!(
                "definitions: {} type(s), {} model(s), {} tool(s), {} function(s), {} agent(s)",
                ir.definitions.types.len(),
                ir.definitions.models.len(),
                ir.definitions.tools.len(),
                ir.definitions.functions.len(),
                ir.definitions.agents.len(),
            );
            println!(
                "graphs: {} graph(s), {node_count} node(s), {edge_count} edge(s)",
                ir.graphs.len()
            );
            Ok(())
        }

        Command::Run { file, target } => {
            let pipeline = compile_frontend(&file)?;
            emit_diagnostics(&pipeline.diagnostics);
            if has_errors(&pipeline.diagnostics) {
                return Err(ExitCode::from(1));
            }

            // Lower AST to IR
            let ir = quew_ir::lower::lower(&pipeline.module, &pipeline.check, &pipeline.interner);

            // Collect all native standard library implementations
            let natives = quew_runtime::native::NativeRegistry::collect();

            // Instantiate and run the interpreter
            let exec = quew_runtime::execution::Execution::new(&ir, &pipeline.interner, &natives);
            match exec.run(&target, quew_runtime::value::Value::Null) {
                Ok(result) => {
                    println!("Execution result: {}", result);
                    Ok(())
                }
                Err(err) => {
                    eprintln!("Execution error: {err}");
                    Err(ExitCode::from(1))
                }
            }
        }
    }
}

struct Pipeline {
    interner: Arc<Interner>,
    parse: quew_parser::ParseResult,
    module: quew_ast::Module,
    check: quew_checker::CheckResult,
    diagnostics: Vec<Diagnostic>,
}

fn compile_frontend(file: &PathBuf) -> Result<Pipeline, ExitCode> {
    let source = fs::read_to_string(file).map_err(|err| {
        eprintln!("failed to read {}: {err}", file.display());
        ExitCode::from(1)
    })?;
    let interner = Arc::new(Interner::new());
    let source_map = SourceMap::new(Arc::clone(&interner));
    let source_id = source_map.add(&file.display().to_string(), source.clone());

    let lex = quew_lexer::lex(&source, source_id, &interner);
    let parse = quew_parser::parse(&lex, &source, &interner);
    let prelude = quew_checker::module_with_prelude(&parse.module, &interner);
    let check = quew_checker::check(&prelude.module, &interner);

    let mut diagnostics = Vec::new();
    diagnostics.extend(lex.errors);
    diagnostics.extend(parse.errors.clone());
    diagnostics.extend(prelude.diagnostics.clone());
    diagnostics.extend(check.diagnostics.clone());

    Ok(Pipeline {
        interner,
        parse,
        module: prelude.module,
        check,
        diagnostics,
    })
}

fn emit_diagnostics(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        eprintln!("{:?}: {}", diagnostic.severity, diagnostic.message);
    }
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}
