use ir_runtime::intent_parser::orchestrator::Orchestrator;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_dir = Path::new("src/intent_parser/fixtures");
    let output_dir = Path::new("src/intent_parser/fixtures-output");

    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    let entries = fs::read_dir(fixtures_dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "yaml") {
            let filename = path.file_name().unwrap().to_str().unwrap();
            let content = fs::read_to_string(&path)?;

            let start = std::time::Instant::now();

            let mut orchestrator = Orchestrator::new(None);
            orchestrator.write(&content);
            let result = orchestrator.end();

            let elapsed = start.elapsed();

            println!("Parsed {} in {:?}", filename, elapsed);

            let output_filename = format!("{}.json", filename.strip_suffix(".yaml").unwrap());
            let output_path = output_dir.join(output_filename);

            fs::write(output_path, serde_json::to_string_pretty(&result)?)?;
        }
    }

    println!("Done! Results saved to {}", output_dir.display());
    Ok(())
}
