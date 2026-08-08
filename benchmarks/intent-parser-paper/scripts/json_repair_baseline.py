#!/usr/bin/env python3
"""Run json_repair as a payload-only comparison on the generated corpus."""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path

import json_repair


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, default=Path("results"))
    args = parser.parse_args()
    corpus_path = args.results / "corpus.jsonl"
    output_path = args.results / "json_repair.csv"

    with corpus_path.open(encoding="utf-8") as source, output_path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(
            target,
            fieldnames=["case_id", "suite", "category", "accepted", "exact_match", "expect_rejection"],
        )
        writer.writeheader()
        for line in source:
            if not line.strip():
                continue
            case = json.loads(line)
            accepted = True
            try:
                parsed = json_repair.loads(case["payload"])
            except (ValueError, TypeError, json.JSONDecodeError):
                accepted = False
                parsed = None
            writer.writerow({
                "case_id": case["id"],
                "suite": case["suite"],
                "category": case["category"],
                "accepted": str(accepted).lower(),
                "exact_match": str(parsed == case.get("expected_args") and case.get("expected_args") is not None).lower(),
                "expect_rejection": str(case.get("expect_rejection", False)).lower(),
            })
    print(f"Wrote {output_path}")


if __name__ == "__main__":
    main()

