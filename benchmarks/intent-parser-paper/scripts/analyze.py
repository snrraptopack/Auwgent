#!/usr/bin/env python3
"""Summarize raw benchmark CSVs and generate paper-ready artifacts."""

from __future__ import annotations

import argparse
import csv
import importlib.metadata
import json
import math
import platform
import statistics
from collections import defaultdict
from pathlib import Path


CONFIG_LABELS = {
    "A_strict_json": "A -- Strict JSON",
    "B_scanner_json": "B -- Stream scanner",
    "C_tolerant_lexer": "C -- Tolerant lexer",
    "D_full_system": "D -- Full system",
}


def truth(value: str) -> bool:
    return value.lower() == "true"


def percentile(values: list[float], q: float) -> float:
    ordered = sorted(values)
    if not ordered:
        return math.nan
    position = (len(ordered) - 1) * q
    low = math.floor(position)
    high = math.ceil(position)
    if low == high:
        return ordered[low]
    return ordered[low] * (high - position) + ordered[high] * (position - low)


def rate_by_repetition(rows: list[dict[str, str]], key: str) -> tuple[float, float]:
    grouped: dict[str, list[bool]] = defaultdict(list)
    for row in rows:
        grouped[row["repetition"]].append(truth(row[key]))
    rates = [100.0 * sum(values) / len(values) for values in grouped.values()]
    return statistics.mean(rates), statistics.pstdev(rates)


def latex_escape(value: str) -> str:
    return value.replace("_", r"\_").replace("%", r"\%")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, default=Path("results"))
    args = parser.parse_args()
    root = args.results.resolve()
    figures = root / "figures"
    figures.mkdir(parents=True, exist_ok=True)

    with (root / "trials.csv").open(newline="", encoding="utf-8") as handle:
        trials = list(csv.DictReader(handle))
    with (root / "transport.csv").open(newline="", encoding="utf-8") as handle:
        transport = list(csv.DictReader(handle))
    metadata = json.loads((root / "metadata.json").read_text(encoding="utf-8"))

    positive = [row for row in trials if not truth(row["expect_rejection"])]
    negative = [row for row in trials if truth(row["expect_rejection"])]
    overall: dict[str, tuple[float, float]] = {}
    per_suite: dict[tuple[str, str], tuple[float, float]] = {}
    over_tolerance: dict[str, float] = {}

    for config in CONFIG_LABELS:
        config_positive = [row for row in positive if row["config"] == config]
        overall[config] = rate_by_repetition(config_positive, "exact_match")
        for suite in sorted({row["suite"] for row in config_positive}):
            suite_rows = [row for row in config_positive if row["suite"] == suite]
            per_suite[(config, suite)] = rate_by_repetition(suite_rows, "exact_match")
        config_negative = [row for row in negative if row["config"] == config]
        over_tolerance[config] = (
            100.0 * sum(truth(row["accepted"]) for row in config_negative) / len(config_negative)
            if config_negative
            else math.nan
        )

    char1 = [row for row in transport if row["mode"] == "char1"]
    transport_invariance = 100.0 * sum(
        truth(row["identical_to_monolithic"]) for row in char1
    ) / len(char1)
    partial_ms = [
        int(row["first_partial_ns"]) / 1_000_000
        for row in char1
        if row["first_partial_ns"]
    ]

    report = [
        "# Intent parser benchmark results",
        "",
        f"Source revision: `{metadata['source_revision']}`  ",
        f"Source patch: `{metadata['source_patch']}` "
        f"(`{metadata['source_patch_sha256']}`)  ",
        f"Rust: `{metadata['rustc']}`  ",
        f"Synthetic cases: {metadata['generated_cases']} "
        f"({metadata['positive_cases']} recoverable, {metadata['rejection_cases']} ambiguity controls)  ",
        f"Repetitions: {metadata['repetitions']}",
        "",
        "## Overall exact syntactic recovery",
        "",
        "| Configuration | SRR (%) | SD across repetitions | Over-tolerance (%) |",
        "|---|---:|---:|---:|",
    ]
    for config, label in CONFIG_LABELS.items():
        mean, sd = overall[config]
        report.append(f"| {label} | {mean:.2f} | {sd:.2f} | {over_tolerance[config]:.2f} |")

    report.extend([
        "",
        "SRR requires exact semantic equality with the expected AST. Over-tolerance is the "
        "acceptance rate on deliberately ambiguous duplicate-key controls, where rejection is the desired behavior.",
        "",
        "## SRR by suite",
        "",
        "| Configuration | Suite A syntax | Suite B protocol | Suite C structure | Suite H holdout |",
        "|---|---:|---:|---:|---:|",
    ])
    for config, label in CONFIG_LABELS.items():
        values = [
            per_suite.get((config, suite), (math.nan, 0.0))[0]
            for suite in ["A", "B", "C", "H"]
        ]
        report.append(
            f"| {label} | {values[0]:.2f} | {values[1]:.2f} | "
            f"{values[2]:.2f} | {values[3]:.2f} |"
        )

    report.extend([
        "",
        "## Streaming",
        "",
        f"- Transport invariance, monolithic vs. one-character chunks: **{transport_invariance:.2f}%**.",
        f"- Parser-side TTFPS for one-character chunks: median **{statistics.median(partial_ms):.4f} ms**, "
        f"p95 **{percentile(partial_ms, 0.95):.4f} ms**.",
        "- TTFPS excludes model generation, network delay, rendering, and tool execution.",
        "",
        "## Scope",
        "",
        "These are deterministic parser micro-benchmarks. No claim about model-scale recovery, "
        "end-to-end first-pass execution, prompt tokens, constrained decoding, or provider-native latency "
        "is supported until real generation and external-baseline data are supplied.",
    ])

    repair_path = root / "json_repair.csv"
    if repair_path.exists():
        with repair_path.open(newline="", encoding="utf-8") as handle:
            repair = list(csv.DictReader(handle))
        eligible = [row for row in repair if row["suite"] in {"A", "C"} and not truth(row["expect_rejection"])]
        repair_srr = 100.0 * sum(truth(row["exact_match"]) for row in eligible) / len(eligible)
        report.extend([
            "",
            "## Repair-library payload baseline",
            "",
            f"`json_repair` exact SRR on Suites A and C payloads: **{repair_srr:.2f}%**. "
            "This payload-only result is not mixed into the cumulative protocol-layer ablation.",
        ])

    (root / "RESULTS.md").write_text("\n".join(report) + "\n", encoding="utf-8")

    tex = [
        "% Generated by scripts/analyze.py; do not edit manually.",
        rf"\newcommand{{\BenchmarkRevision}}{{\texttt{{{metadata['source_revision'][:7]}}}}}",
        rf"\newcommand{{\BenchmarkCaseCount}}{{{metadata['generated_cases']}}}",
        rf"\newcommand{{\TransportInvariance}}{{{transport_invariance:.2f}\%}}",
        rf"\newcommand{{\ParserTTFPSMedian}}{{{statistics.median(partial_ms):.4f}}}",
        rf"\newcommand{{\ParserTTFPSP95}}{{{percentile(partial_ms, 0.95):.4f}}}",
    ]
    for config, macro in [
        ("A_strict_json", "StrictSRR"),
        ("B_scanner_json", "ScannerSRR"),
        ("C_tolerant_lexer", "TolerantSRR"),
        ("D_full_system", "FullSRR"),
    ]:
        mean, sd = overall[config]
        tex.append(rf"\newcommand{{\{macro}}}{{{mean:.2f}\%}}")
        tex.append(rf"\newcommand{{\{macro}SD}}{{{sd:.2f}}}")
    tex.extend([
        r"\begin{tabular}{@{}lrr@{}}",
        r"\toprule",
        r"Configuration & SRR (\%) & SD \\",
        r"\midrule",
    ])
    for config, label in CONFIG_LABELS.items():
        mean, sd = overall[config]
        tex.append(rf"{latex_escape(label)} & {mean:.2f} & {sd:.2f} \\")
    tex.extend([r"\bottomrule", r"\end{tabular}"])
    (root / "paper_results.tex").write_text("\n".join(tex) + "\n", encoding="utf-8")

    analysis_metadata = {
        "python": platform.python_version(),
        "matplotlib": importlib.metadata.version("matplotlib"),
        "json_repair": importlib.metadata.version("json-repair"),
    }
    (root / "analysis_metadata.json").write_text(
        json.dumps(analysis_metadata, indent=2) + "\n", encoding="utf-8"
    )

    draw_figures(figures, overall, transport, trials)
    print(f"Wrote {root / 'RESULTS.md'}")
    print(f"Wrote {root / 'paper_results.tex'}")


def draw_figures(
    figures: Path,
    overall: dict[str, tuple[float, float]],
    transport: list[dict[str, str]],
    trials: list[dict[str, str]],
) -> None:
    import matplotlib.pyplot as plt

    configs = list(CONFIG_LABELS)
    means = [overall[config][0] for config in configs]
    errors = [overall[config][1] for config in configs]
    labels = [CONFIG_LABELS[config].split(" -- ", 1)[0] for config in configs]
    fig, ax = plt.subplots(figsize=(6.4, 3.8))
    bars = ax.bar(labels, means, yerr=errors, capsize=3, color=["#8c8c8c", "#5b8db8", "#50a37f", "#df8f44"])
    ax.set_ylabel("Exact syntactic recovery rate (%)")
    ax.set_ylim(0, 105)
    ax.grid(axis="y", alpha=0.25)
    for bar, value in zip(bars, means):
        ax.text(bar.get_x() + bar.get_width() / 2, value + 1.2, f"{value:.1f}", ha="center", fontsize=8)
    fig.tight_layout()
    fig.savefig(figures / "factorial_ablation.pdf")
    fig.savefig(figures / "factorial_ablation.png", dpi=220)
    plt.close(fig)

    fig, ax = plt.subplots(figsize=(6.4, 3.8))
    for mode, label in [("char1", "1 character"), ("char5", "5 characters"), ("random", "Random"), ("monolithic", "Monolithic")]:
        values = sorted(int(row["elapsed_ns"]) / 1_000_000 for row in transport if row["mode"] == mode)
        y = [(index + 1) / len(values) for index in range(len(values))]
        ax.plot(values, y, label=label)
    ax.set_xlabel("Parser execution latency (ms)")
    ax.set_ylabel("Empirical CDF")
    ax.grid(alpha=0.25)
    ax.legend()
    fig.tight_layout()
    fig.savefig(figures / "transport_latency_cdf.pdf")
    fig.savefig(figures / "transport_latency_cdf.png", dpi=220)
    plt.close(fig)

    model_rows = [row for row in trials if row.get("model_scale")]
    if model_rows:
        by_scale: dict[str, list[bool]] = defaultdict(list)
        for row in model_rows:
            if row["config"] == "D_full_system":
                by_scale[row["model_scale"]].append(truth(row["exact_match"]))
        ordered = sorted(by_scale, key=lambda value: float(value.rstrip("Bb")))
        rates = [100.0 * sum(by_scale[scale]) / len(by_scale[scale]) for scale in ordered]
        fig, ax = plt.subplots(figsize=(6.4, 3.8))
        ax.plot(ordered, rates, marker="o")
        ax.set_xlabel("Model parameter scale")
        ax.set_ylabel("Full-system recovery rate (%)")
        ax.set_ylim(0, 105)
        ax.grid(alpha=0.25)
        fig.tight_layout()
        fig.savefig(figures / "recovery_by_model_scale.pdf")
        fig.savefig(figures / "recovery_by_model_scale.png", dpi=220)
        plt.close(fig)


if __name__ == "__main__":
    main()
