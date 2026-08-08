# Auwgent intent-parser paper benchmark

This directory is a copyable reproduction artifact for the parser evaluation.
It measures immutable Auwgent Git revision
`5a110e78a8440921d7d4302769bc049180f9d2bf` (`v0.1.37`) plus the included
`patches/parser-hardening.patch` (SHA-256 recorded in `results/metadata.json`),
not whichever parser happens to be present in a surrounding checkout.

## What is included

- Suite A: 350 deterministic syntax-corruption cases (100 unquoted, 100
  layout, 100 missing-comma, 50 protected-special-character cases).
- Suite B: 50 unclosed headers, 50 missing closing tags, and 100 valid streams
  delivered monolithically, one character at a time, five characters at a
  time, and at seeded-random boundaries.
- Suite C: 100 dot-notation cases at depths 1--5 and 50 compiler-alias cases.
- Suite E: 100 conflicting duplicate-key ambiguity controls.
- Suite H: 200 independent holdout cases: 50 unquoted emails, 50 unclosed
  header variants, 50 schema-known dotted paths, and 50 duplicate controls.
- Four ablations: strict envelope + JSON, scanner + JSON, scanner + tolerant
  lexer, and the full orchestrator with compiler-derived unflattening.
- Raw CSV/JSONL output, semantic exact-match scoring, charts, Markdown results,
  and generated LaTeX macros/table rows.
- Optional `json_repair` payload baseline and an importer format for captured
  real-model generations.

Every recovery success requires exact equality with the expected AST. Merely
returning some AST is recorded as acceptance, not correctness.

## Run

Requirements: Rust stable, Python 3.12, `curl`, and `tar`.
[`uv`](https://docs.astral.sh/uv/) is the preferred Python runner; `uv.lock`
pins the analysis and repair-baseline environment.

Windows PowerShell:

```powershell
.\run.ps1 --out results --repetitions 10
uv run python scripts\json_repair_baseline.py --results results
uv run python scripts\analyze.py --results results
```

Linux/macOS:

```sh
chmod +x run.sh scripts/prepare-source.sh
./run.sh --out results --repetitions 10
uv run python scripts/json_repair_baseline.py --results results
uv run python scripts/analyze.py --results results
```

The first run downloads the pinned GitHub source archive into `vendor/` and
applies the included hardening patch after verifying its applicability.
Thereafter, the folder is self-contained apart from already locked crates.io
dependencies. Copying the entire directory after the first run preserves the
measured source and the raw results.

Use `cargo test` after source preparation to validate cardinalities, holdout
outcomes, alias unflattening, and transport invariance.

## Outputs

- `results/corpus.jsonl`: every generated input and oracle.
- `results/trials.csv`: raw per-case, per-configuration, per-repetition data.
- `results/summary.csv`: machine-readable aggregates.
- `results/transport.csv`: all 4,000 chunk-sweep trials at 10 repetitions.
- `results/metadata.json`: base revision, patch identity/hash, environment,
  and metric scope.
- `results/analysis_metadata.json`: exact Python analysis-package versions.
- `results/RESULTS.md`: paper-readable findings and limitations.
- `results/paper_results.tex`: generated LaTeX macros and ablation table body.
- `results/figures/`: PDF and PNG figures.
- `CHECKSUMS.sha256`: integrity manifest for the archived code, raw data,
  summaries, and paper PDFs from the included run. Rerunning timing trials is
  expected to change result hashes.

## Metric boundaries

The reported SRR is semantic exact-match SRR. The parser TTFPS clock begins
when the harness supplies the first chunk and ends at the first valid partial
intent callback; it excludes model generation, network latency, UI rendering,
and tool execution. Consequently it must not be presented as end-to-end TTFPS.

The deterministic harness does not establish recovery by model parameter
scale, true first-pass tool execution, prompt-token reduction, or latency
against XGrammar/provider-native tools. Those claims require external model
inference or previously published evidence. A model-scale chart is generated
only when labeled real-generation rows are supplied with `--real-world`.

The transport CDF compares parser cost under chunking modes. It is not a CDF
comparison against constrained decoding or provider-native execution.

## Real-world suite

Pass captured generations without making API calls from the harness:

```powershell
.\run.ps1 --real-world data\real_world\generations.jsonl
```

See `data/real_world/README.md` for the row schema and required provenance.
