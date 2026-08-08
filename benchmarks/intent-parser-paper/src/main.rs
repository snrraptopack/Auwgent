use auwgent_protocol::BlockOrchestrator;
use function_parser::{
    ASTValue, BlockScanner, BlockType, parse_assignment_object, parse_ts_object,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const SOURCE_REVISION: &str = "5a110e78a8440921d7d4302769bc049180f9d2bf";
const SOURCE_PATCH: &str = "parser-hardening-v1";
const SOURCE_PATCH_SHA256: &str =
    "4ed8e986cdcd0c47dc9bea181be2371835bc065c05302de61b51c96fa7fc31cf";
const DEFAULT_REPETITIONS: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Case {
    id: String,
    suite: String,
    category: String,
    payload: String,
    stream: String,
    expected_args: Option<Value>,
    schema: Value,
    #[serde(default)]
    expect_rejection: bool,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    model_scale: Option<String>,
}

#[derive(Clone, Copy, Debug)]
enum Config {
    Strict,
    ScannerStrict,
    ScannerTolerant,
    Full,
}

impl Config {
    const ALL: [Self; 4] = [
        Self::Strict,
        Self::ScannerStrict,
        Self::ScannerTolerant,
        Self::Full,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Strict => "A_strict_json",
            Self::ScannerStrict => "B_scanner_json",
            Self::ScannerTolerant => "C_tolerant_lexer",
            Self::Full => "D_full_system",
        }
    }
}

#[derive(Debug)]
struct RunOutcome {
    accepted: bool,
    parsed: Option<Value>,
    elapsed_ns: u128,
    first_partial_ns: Option<u128>,
    partial_count: usize,
}

#[derive(Serialize)]
struct TrialRow<'a> {
    case_id: &'a str,
    suite: &'a str,
    category: &'a str,
    config: &'a str,
    repetition: usize,
    accepted: bool,
    exact_match: bool,
    desired_outcome: bool,
    expect_rejection: bool,
    elapsed_ns: u128,
    first_partial_ns: Option<u128>,
    model: Option<&'a str>,
    model_scale: Option<&'a str>,
}

#[derive(Default, Serialize)]
struct Aggregate {
    attempts: usize,
    accepted: usize,
    exact_matches: usize,
    desired_outcomes: usize,
    elapsed_ns_sum: u128,
    first_partial_count: usize,
    first_partial_ns_sum: u128,
}

#[derive(Serialize)]
struct Metadata {
    artifact: &'static str,
    source_repository: &'static str,
    source_revision: &'static str,
    source_patch: &'static str,
    source_patch_sha256: &'static str,
    repetitions: usize,
    generated_cases: usize,
    positive_cases: usize,
    rejection_cases: usize,
    rustc: String,
    operating_system: String,
    architecture: String,
    metric_note: &'static str,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::parse()?;
    fs::create_dir_all(&options.out)?;

    let mut cases = synthetic_cases();
    if let Some(path) = &options.real_world {
        cases.extend(read_jsonl_cases(path)?);
    }

    write_jsonl(&options.out.join("corpus.jsonl"), &cases)?;
    run_trials(&options.out, &cases, options.repetitions)?;
    run_transport_sweep(&options.out, options.repetitions)?;

    let metadata = Metadata {
        artifact: "Auwgent IMP parser paper benchmark",
        source_repository: "https://github.com/snrraptopack/Auwgent",
        source_revision: SOURCE_REVISION,
        source_patch: SOURCE_PATCH,
        source_patch_sha256: SOURCE_PATCH_SHA256,
        repetitions: options.repetitions,
        generated_cases: cases.len(),
        positive_cases: cases.iter().filter(|case| !case.expect_rejection).count(),
        rejection_cases: cases.iter().filter(|case| case.expect_rejection).count(),
        rustc: rustc_version(),
        operating_system: env::consts::OS.to_string(),
        architecture: env::consts::ARCH.to_string(),
        metric_note: "TTFPS is parser-side time from first supplied chunk to first valid partial callback; it excludes model and network latency.",
    };
    write_pretty_json(&options.out.join("metadata.json"), &metadata)?;

    println!(
        "Wrote reproducible raw results to {}",
        options.out.display()
    );
    println!(
        "Next: python scripts/analyze.py --results {}",
        options.out.display()
    );
    Ok(())
}

struct Options {
    out: PathBuf,
    repetitions: usize,
    real_world: Option<PathBuf>,
}

impl Options {
    fn parse() -> Result<Self, String> {
        let mut out = PathBuf::from("results");
        let mut repetitions = DEFAULT_REPETITIONS;
        let mut real_world = None;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--out" => out = PathBuf::from(args.next().ok_or("--out needs a path")?),
                "--repetitions" => {
                    repetitions = args
                        .next()
                        .ok_or("--repetitions needs an integer")?
                        .parse()
                        .map_err(|_| "invalid --repetitions")?;
                    if repetitions == 0 {
                        return Err("--repetitions must be positive".to_string());
                    }
                }
                "--real-world" => {
                    real_world = Some(PathBuf::from(
                        args.next().ok_or("--real-world needs a JSONL path")?,
                    ));
                }
                "-h" | "--help" => {
                    println!(
                        "Usage: cargo run --release -- [--out results] [--repetitions 10] [--real-world generations.jsonl]"
                    );
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        Ok(Self {
            out,
            repetitions,
            real_world,
        })
    }
}

fn synthetic_cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let flat_schema = json!({
        "name": field("string"),
        "age": field("number")
    });

    for i in 0..100 {
        let name = format!("Alice_{i:03}");
        let age = 18 + (i % 63);
        let payload = if i % 2 == 0 {
            format!("{{name: {name}, age: {age}}}")
        } else {
            format!("{{name: '{name}', age: {age}}}")
        };
        cases.push(positive_case(
            format!("A-unquoted-{i:03}"),
            "A",
            "unquoted_keys_and_strings",
            payload,
            json!({"name": name, "age": age}),
            flat_schema.clone(),
        ));
    }

    for i in 0..100 {
        let name = format!("Layout User {i:03}");
        let age = 20 + (i % 50);
        cases.push(positive_case(
            format!("A-layout-{i:03}"),
            "A",
            "layout_indented",
            format!("name: {name}\nage: {age}"),
            json!({"name": name, "age": age}),
            flat_schema.clone(),
        ));
    }

    for i in 0..100 {
        let name = format!("Comma_{i:03}");
        let age = 21 + (i % 40);
        cases.push(positive_case(
            format!("A-comma-{i:03}"),
            "A",
            "missing_commas",
            format!("{{name: {name} age: {age}}}"),
            json!({"name": name, "age": age}),
            flat_schema.clone(),
        ));
    }

    for i in 0..50 {
        let (kind, value) = match i % 4 {
            0 => ("email_at", format!("user{i:03}@example.com")),
            1 => ("url_dots", format!("https://api.example.com/items/{i}")),
            2 => ("embedded_colon", format!("region: Tarkwa case {i:03}")),
            _ => ("hyphenated", format!("intent-parser-case-{i:03}")),
        };
        cases.push(positive_case(
            format!("A-special-{i:03}"),
            "A",
            &format!("special_character_{kind}"),
            format!("value: {value}"),
            json!({"value": value}),
            json!({"value": field("string")}),
        ));
    }

    for i in 0..50 {
        let expected = json!({"name": format!("Header_{i:03}"), "age": 30 + i});
        let payload = serde_json::to_string(&expected).unwrap();
        cases.push(Case {
            id: format!("B-header-{i:03}"),
            suite: "B".to_string(),
            category: "unclosed_header_bracket".to_string(),
            stream: format!("[tool_call: bench\n{payload}\n[/tool_call]"),
            payload,
            expected_args: Some(expected),
            schema: flat_schema.clone(),
            expect_rejection: false,
            model: None,
            model_scale: None,
        });
    }

    for i in 0..50 {
        let expected = json!({"name": format!("Tail_{i:03}"), "age": 40 + i});
        let payload = serde_json::to_string(&expected).unwrap();
        cases.push(Case {
            id: format!("B-tail-{i:03}"),
            suite: "B".to_string(),
            category: "missing_closing_tag".to_string(),
            stream: format!("[tool_call: bench]\n{payload}"),
            payload,
            expected_args: Some(expected),
            schema: flat_schema.clone(),
            expect_rejection: false,
            model: None,
            model_scale: None,
        });
    }

    for i in 0..100 {
        let depth = 1 + (i % 5);
        let path = path_for_depth(depth);
        let value = format!("Tarkwa_{i:03}");
        let payload = format!("{}: {}", path.join("."), value);
        cases.push(positive_case(
            format!("C-dot-{i:03}"),
            "C",
            "dot_notation",
            payload,
            nested_value(&path, Value::String(value)),
            schema_for_path(&path),
        ));
    }

    for i in 0..50 {
        let depth = 2 + (i % 4);
        let path = path_for_depth(depth);
        let value = format!("Alias_{i:03}");
        let payload = format!("{}: {}", path.join("_"), value);
        cases.push(positive_case(
            format!("C-alias-{i:03}"),
            "C",
            "compiler_alias",
            payload,
            nested_value(&path, Value::String(value)),
            schema_for_path(&path),
        ));
    }

    for i in 0..100 {
        let payload = format!("{{\"name\":\"first_{i:03}\",\"name\":\"second_{i:03}\"}}");
        cases.push(Case {
            id: format!("E-duplicate-{i:03}"),
            suite: "E".to_string(),
            category: "conflicting_duplicate_key".to_string(),
            stream: wrap(&payload),
            payload,
            expected_args: None,
            schema: json!({"name": field("string")}),
            expect_rejection: true,
            model: None,
            model_scale: None,
        });
    }

    // Fix-validation holdout: these templates and values are distinct from the
    // motivating cases above, while exercising the same four fault classes.
    for i in 0..50 {
        let local = match i % 3 {
            0 => format!("alerts+paper{i:03}"),
            1 => format!("first.last-{i:03}"),
            _ => format!("parser_{i:03}"),
        };
        let domain = match i % 4 {
            0 => "research.example.org",
            1 => "mail.sub-domain.test",
            2 => "tools.example.co.uk",
            _ => "streaming.dev",
        };
        let value = format!("{local}@{domain}");
        cases.push(positive_case(
            format!("H-email-{i:03}"),
            "H",
            "holdout_unquoted_email",
            format!("contact: {value}"),
            json!({"contact": value}),
            json!({"contact": field("string")}),
        ));
    }

    for i in 0..50 {
        let expected = json!({"request": format!("holdout_{i:03}"), "limit": 1 + i});
        let payload = serde_json::to_string(&expected).unwrap();
        let stream = if i % 2 == 0 {
            format!("[tool_call:bench\n{payload}\n[/tool_call]")
        } else {
            format!("[tool_call: bench  \r\n{payload}\r\n[/tool_call]")
        };
        cases.push(Case {
            id: format!("H-header-{i:03}"),
            suite: "H".to_string(),
            category: "holdout_unclosed_header".to_string(),
            payload,
            stream,
            expected_args: Some(expected),
            schema: json!({"request": field("string"), "limit": field("number")}),
            expect_rejection: false,
            model: None,
            model_scale: None,
        });
    }

    let holdout_paths = [
        &["account", "profile"][..],
        &["customer", "contact", "email"][..],
        &["order", "shipping", "address", "city"][..],
        &["workspace", "team", "member", "profile", "handle"][..],
    ];
    for i in 0..50 {
        let path: Vec<String> = holdout_paths[i % holdout_paths.len()]
            .iter()
            .map(|segment| (*segment).to_string())
            .collect();
        let value = format!("holdout_value_{i:03}");
        cases.push(positive_case(
            format!("H-dot-{i:03}"),
            "H",
            "holdout_dot_notation",
            format!("{}: {value}", path.join(".")),
            nested_value(&path, Value::String(value)),
            schema_for_path(&path),
        ));
    }

    for i in 0..50 {
        let first = format!("first_holdout_{i:03}");
        let second = format!("second_holdout_{i:03}");
        let payload = match i % 4 {
            0 => format!("{{\"name\":\"{first}\",\"name\":\"{second}\"}}"),
            1 => format!("{{name: '{first}', name: '{second}'}}"),
            2 => format!("{{name: {first}, name: {second}}}"),
            _ => format!("name: {first}\nname: {second}"),
        };
        cases.push(Case {
            id: format!("H-duplicate-{i:03}"),
            suite: "H".to_string(),
            category: "holdout_duplicate_key".to_string(),
            stream: wrap(&payload),
            payload,
            expected_args: None,
            schema: json!({"name": field("string")}),
            expect_rejection: true,
            model: None,
            model_scale: None,
        });
    }

    cases
}

fn positive_case(
    id: String,
    suite: &str,
    category: &str,
    payload: String,
    expected: Value,
    schema: Value,
) -> Case {
    Case {
        id,
        suite: suite.to_string(),
        category: category.to_string(),
        stream: wrap(&payload),
        payload,
        expected_args: Some(expected),
        schema,
        expect_rejection: false,
        model: None,
        model_scale: None,
    }
}

fn wrap(payload: &str) -> String {
    format!("[tool_call: bench]\n{payload}\n[/tool_call]")
}

fn field(kind: &str) -> Value {
    json!({"type": kind, "optional": false})
}

fn path_for_depth(depth: usize) -> Vec<String> {
    const SEGMENTS: [&str; 5] = ["user", "address", "region", "district", "city"];
    SEGMENTS[..depth].iter().map(|s| s.to_string()).collect()
}

fn nested_value(path: &[String], leaf: Value) -> Value {
    let mut value = leaf;
    for segment in path.iter().rev() {
        value = json!({segment: value});
    }
    value
}

fn schema_for_path(path: &[String]) -> Value {
    let mut definition = field("string");
    for segment in path.iter().skip(1).rev() {
        definition = json!({
            "type": {"type": "object", "properties": {segment: definition}},
            "optional": false
        });
    }
    json!({path[0].clone(): definition})
}

fn run_trials(
    out: &Path,
    cases: &[Case],
    repetitions: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut trials = csv_writer(&out.join("trials.csv"))?;
    writeln!(
        trials,
        "case_id,suite,category,config,repetition,accepted,exact_match,desired_outcome,expect_rejection,elapsed_ns,first_partial_ns,model,model_scale"
    )?;
    let mut aggregates: BTreeMap<(String, String, String), Aggregate> = BTreeMap::new();

    for case in cases {
        for config in Config::ALL {
            for repetition in 0..repetitions {
                let outcome = run_config(config, case, &[case.stream.clone()]);
                let exact_match = case
                    .expected_args
                    .as_ref()
                    .is_some_and(|expected| outcome.parsed.as_ref() == Some(expected));
                let desired = if case.expect_rejection {
                    !outcome.accepted
                } else {
                    exact_match
                };
                let row = TrialRow {
                    case_id: &case.id,
                    suite: &case.suite,
                    category: &case.category,
                    config: config.id(),
                    repetition,
                    accepted: outcome.accepted,
                    exact_match,
                    desired_outcome: desired,
                    expect_rejection: case.expect_rejection,
                    elapsed_ns: outcome.elapsed_ns,
                    first_partial_ns: outcome.first_partial_ns,
                    model: case.model.as_deref(),
                    model_scale: case.model_scale.as_deref(),
                };
                write_trial(&mut trials, &row)?;

                let key = (
                    config.id().to_string(),
                    case.suite.clone(),
                    case.category.clone(),
                );
                let agg = aggregates.entry(key).or_default();
                agg.attempts += 1;
                agg.accepted += usize::from(outcome.accepted);
                agg.exact_matches += usize::from(exact_match);
                agg.desired_outcomes += usize::from(desired);
                agg.elapsed_ns_sum += outcome.elapsed_ns;
                if let Some(ns) = outcome.first_partial_ns {
                    agg.first_partial_count += 1;
                    agg.first_partial_ns_sum += ns;
                }
            }
        }
    }
    trials.flush()?;

    let mut summary = csv_writer(&out.join("summary.csv"))?;
    writeln!(
        summary,
        "config,suite,category,attempts,accepted,exact_matches,desired_outcomes,srr_pct,acceptance_pct,mean_elapsed_ns,mean_first_partial_ns"
    )?;
    for ((config, suite, category), agg) in &aggregates {
        let srr = 100.0 * agg.exact_matches as f64 / agg.attempts as f64;
        let acceptance = 100.0 * agg.accepted as f64 / agg.attempts as f64;
        let mean_elapsed = agg.elapsed_ns_sum as f64 / agg.attempts as f64;
        let mean_partial = if agg.first_partial_count == 0 {
            String::new()
        } else {
            format!(
                "{:.3}",
                agg.first_partial_ns_sum as f64 / agg.first_partial_count as f64
            )
        };
        writeln!(
            summary,
            "{config},{suite},{category},{},{},{},{},{srr:.6},{acceptance:.6},{mean_elapsed:.3},{mean_partial}",
            agg.attempts, agg.accepted, agg.exact_matches, agg.desired_outcomes
        )?;
    }
    summary.flush()?;
    Ok(())
}

fn run_config(config: Config, case: &Case, chunks: &[String]) -> RunOutcome {
    match config {
        Config::Strict => run_strict(case),
        Config::ScannerStrict => run_scanner(case, false),
        Config::ScannerTolerant => run_scanner(case, true),
        Config::Full => run_full(case, chunks),
    }
}

fn run_strict(case: &Case) -> RunOutcome {
    let start = Instant::now();
    let parsed = exact_envelope_payload(&case.stream)
        .and_then(|payload| serde_json::from_str::<Value>(payload).ok());
    RunOutcome {
        accepted: parsed.is_some(),
        parsed,
        elapsed_ns: start.elapsed().as_nanos(),
        first_partial_ns: None,
        partial_count: 0,
    }
}

fn exact_envelope_payload(stream: &str) -> Option<&str> {
    let prefix = "[tool_call: bench]\n";
    let suffix = "\n[/tool_call]";
    stream.strip_prefix(prefix)?.strip_suffix(suffix)
}

fn run_scanner(case: &Case, tolerant: bool) -> RunOutcome {
    let start = Instant::now();
    let mut scanner = BlockScanner::new_final(&case.stream);
    let parsed = scanner
        .scan()
        .into_iter()
        .find(|block| {
            block.block_type == BlockType::Tool && block.target_name.as_deref() == Some("bench")
        })
        .and_then(|block| {
            if tolerant {
                parse_payload_tolerant(&block.content)
            } else {
                serde_json::from_str::<Value>(&block.content).ok()
            }
        });
    RunOutcome {
        accepted: parsed.is_some(),
        parsed,
        elapsed_ns: start.elapsed().as_nanos(),
        first_partial_ns: None,
        partial_count: 0,
    }
}

fn parse_payload_tolerant(payload: &str) -> Option<Value> {
    let trimmed = payload.trim();
    let ast = if trimmed.starts_with('{') {
        parse_ts_object(trimmed).ok()?
    } else {
        ASTValue::Object(parse_assignment_object(trimmed).ok()?)
    };
    Some(ast_to_json(&ast))
}

fn ast_to_json(value: &ASTValue) -> Value {
    match value {
        ASTValue::String(value) => Value::String(value.clone()),
        ASTValue::Number(value) => {
            let number = if value.is_finite() && value.fract() == 0.0 {
                if *value >= 0.0 && *value <= u64::MAX as f64 {
                    Some(serde_json::Number::from(*value as u64))
                } else if *value >= i64::MIN as f64 && *value <= i64::MAX as f64 {
                    Some(serde_json::Number::from(*value as i64))
                } else {
                    serde_json::Number::from_f64(*value)
                }
            } else {
                serde_json::Number::from_f64(*value)
            };
            number.map(Value::Number).unwrap_or(Value::Null)
        }
        ASTValue::Boolean(value) => Value::Bool(*value),
        ASTValue::Array(values) => Value::Array(values.iter().map(ast_to_json).collect()),
        ASTValue::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), ast_to_json(value)))
                .collect(),
        ),
        ASTValue::Call { name, args } => json!({
            "__kind": "call",
            "name": name,
            "args": args.iter().map(|(key, value)| (key.clone(), ast_to_json(value))).collect::<Map<String, Value>>()
        }),
        ASTValue::Null => Value::Null,
    }
}

fn run_full(case: &Case, chunks: &[String]) -> RunOutcome {
    let finals = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let partials = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let first_partial = Arc::new(Mutex::new(None::<u128>));
    let start_cell = Arc::new(Mutex::new(None::<Instant>));

    let mut orchestrator = BlockOrchestrator::new();
    orchestrator.register_intent("tool_call");
    orchestrator.register_tool_shape("bench", &case.schema, None);

    let finals_out = Arc::clone(&finals);
    orchestrator.on_intent_ready(Arc::new(move |name, value| {
        finals_out.lock().unwrap().push((name, value));
    }));

    let partials_out = Arc::clone(&partials);
    let first_partial_out = Arc::clone(&first_partial);
    let start_out = Arc::clone(&start_cell);
    orchestrator.on_intent_partial(Arc::new(move |name, value| {
        if let Some(start) = *start_out.lock().unwrap() {
            let mut first = first_partial_out.lock().unwrap();
            if first.is_none() && name == "tool_call" {
                *first = Some(start.elapsed().as_nanos());
            }
        }
        partials_out.lock().unwrap().push((name, value));
    }));

    let start = Instant::now();
    *start_cell.lock().unwrap() = Some(start);
    for chunk in chunks {
        orchestrator.write(chunk);
    }
    orchestrator.end();
    let elapsed_ns = start.elapsed().as_nanos();

    let parsed = finals
        .lock()
        .unwrap()
        .iter()
        .find(|(name, value)| {
            name == "tool_call" && value.get("type").and_then(Value::as_str) == Some("bench")
        })
        .and_then(|(_, value)| value.get("args").cloned());
    let partial_count = partials.lock().unwrap().len();
    let first_partial_ns = *first_partial.lock().unwrap();
    RunOutcome {
        accepted: parsed.is_some(),
        parsed,
        elapsed_ns,
        first_partial_ns,
        partial_count,
    }
}

fn run_transport_sweep(out: &Path, repetitions: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = csv_writer(&out.join("transport.csv"))?;
    writeln!(
        writer,
        "case_id,mode,repetition,identical_to_monolithic,exact_match,elapsed_ns,first_partial_ns,partial_count,chunks"
    )?;
    let schema = json!({"name": field("string"), "age": field("number")});

    for i in 0..100 {
        let expected = json!({"name": format!("Transport_{i:03}"), "age": 20 + i});
        let payload = format!("name: Transport_{i:03}\nage: {}", 20 + i);
        let case = positive_case(
            format!("B-transport-{i:03}"),
            "B",
            "transport_chunk_sweep",
            payload,
            expected.clone(),
            schema.clone(),
        );
        let baseline = run_full(&case, &[case.stream.clone()]).parsed;
        for mode in ["char1", "char5", "random", "monolithic"] {
            let chunks = chunks_for(&case.stream, mode, 0xA17E_0000 + i as u64);
            for repetition in 0..repetitions {
                let outcome = run_full(&case, &chunks);
                let identical = outcome.parsed == baseline;
                let exact = outcome.parsed.as_ref() == Some(&expected);
                writeln!(
                    writer,
                    "{},{},{},{},{},{},{},{},{}",
                    case.id,
                    mode,
                    repetition,
                    identical,
                    exact,
                    outcome.elapsed_ns,
                    outcome
                        .first_partial_ns
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    outcome.partial_count,
                    chunks.len()
                )?;
            }
        }
    }
    writer.flush()?;
    Ok(())
}

fn chunks_for(input: &str, mode: &str, seed: u64) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    if mode == "monolithic" {
        return vec![input.to_string()];
    }
    let mut chunks = Vec::new();
    let mut index = 0;
    let mut state = seed;
    while index < chars.len() {
        let size = match mode {
            "char1" => 1,
            "char5" => 5,
            "random" => {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                1 + ((state >> 32) as usize % 17)
            }
            _ => unreachable!(),
        };
        let end = (index + size).min(chars.len());
        chunks.push(chars[index..end].iter().collect());
        index = end;
    }
    chunks
}

fn write_trial(writer: &mut BufWriter<File>, row: &TrialRow<'_>) -> std::io::Result<()> {
    writeln!(
        writer,
        "{},{},{},{},{},{},{},{},{},{},{},{},{}",
        row.case_id,
        row.suite,
        row.category,
        row.config,
        row.repetition,
        row.accepted,
        row.exact_match,
        row.desired_outcome,
        row.expect_rejection,
        row.elapsed_ns,
        row.first_partial_ns
            .map(|v| v.to_string())
            .unwrap_or_default(),
        row.model.unwrap_or(""),
        row.model_scale.unwrap_or("")
    )
}

fn read_jsonl_cases(path: &Path) -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let reader = BufReader::new(File::open(path)?);
    let mut cases = Vec::new();
    for (line_number, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let case: Case = serde_json::from_str(&line)
            .map_err(|error| format!("{}:{}: {error}", path.display(), line_number + 1))?;
        cases.push(case);
    }
    Ok(cases)
}

fn write_jsonl(path: &Path, cases: &[Case]) -> Result<(), Box<dyn std::error::Error>> {
    let mut writer = BufWriter::new(File::create(path)?);
    for case in cases {
        serde_json::to_writer(&mut writer, case)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn write_pretty_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let writer = BufWriter::new(File::create(path)?);
    serde_json::to_writer_pretty(writer, value)?;
    Ok(())
}

fn csv_writer(path: &Path) -> std::io::Result<BufWriter<File>> {
    Ok(BufWriter::new(File::create(path)?))
}

fn rustc_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_has_declared_cardinality() {
        let cases = synthetic_cases();
        assert_eq!(cases.iter().filter(|c| c.suite == "A").count(), 350);
        assert_eq!(cases.iter().filter(|c| c.suite == "B").count(), 100);
        assert_eq!(cases.iter().filter(|c| c.suite == "C").count(), 150);
        assert_eq!(cases.iter().filter(|c| c.suite == "E").count(), 100);
        assert_eq!(cases.iter().filter(|c| c.suite == "H").count(), 200);
        assert_eq!(cases.iter().filter(|c| !c.expect_rejection).count(), 750);
        assert_eq!(cases.iter().filter(|c| c.expect_rejection).count(), 150);
    }

    #[test]
    fn alias_case_is_reconstructed_only_by_full_system() {
        let case = synthetic_cases()
            .into_iter()
            .find(|case| case.category == "compiler_alias")
            .unwrap();
        assert_ne!(run_scanner(&case, true).parsed, case.expected_args);
        assert_eq!(
            run_full(&case, &[case.stream.clone()]).parsed,
            case.expected_args
        );
    }

    #[test]
    fn chunking_is_invariant_for_valid_stream() {
        let case = positive_case(
            "transport-test".to_string(),
            "B",
            "transport_chunk_sweep",
            "name: Alice\nage: 20".to_string(),
            json!({"name": "Alice", "age": 20}),
            json!({"name": field("string"), "age": field("number")}),
        );
        let mono = run_full(&case, &[case.stream.clone()]).parsed;
        let one = run_full(&case, &chunks_for(&case.stream, "char1", 1)).parsed;
        assert_eq!(mono, one);
    }

    #[test]
    fn full_system_satisfies_every_holdout_case() {
        for case in synthetic_cases()
            .into_iter()
            .filter(|case| case.suite == "H")
        {
            let outcome = run_full(&case, &[case.stream.clone()]);
            if case.expect_rejection {
                assert!(!outcome.accepted, "{} was unexpectedly accepted", case.id);
            } else {
                assert_eq!(
                    outcome.parsed, case.expected_args,
                    "{} did not produce the expected AST",
                    case.id
                );
            }
        }
    }
}
