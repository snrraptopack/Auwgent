# Intent parser benchmark results

Source revision: `5a110e78a8440921d7d4302769bc049180f9d2bf`  
Source patch: `parser-hardening-v1` (`4ed8e986cdcd0c47dc9bea181be2371835bc065c05302de61b51c96fa7fc31cf`)  
Rust: `rustc 1.93.1 (01f6ddf75 2026-02-11)`  
Synthetic cases: 900 (750 recoverable, 150 ambiguity controls)  
Repetitions: 10

## Overall exact syntactic recovery

| Configuration | SRR (%) | SD across repetitions | Over-tolerance (%) |
|---|---:|---:|---:|
| A -- Strict JSON | 0.00 | 0.00 | 75.33 |
| B -- Stream scanner | 20.00 | 0.00 | 75.33 |
| C -- Tolerant lexer | 76.00 | 0.00 | 0.00 |
| D -- Full system | 100.00 | 0.00 | 0.00 |

SRR requires exact semantic equality with the expected AST. Over-tolerance is the acceptance rate on deliberately ambiguous duplicate-key controls, where rejection is the desired behavior.

## SRR by suite

| Configuration | Suite A syntax | Suite B protocol | Suite C structure | Suite H holdout |
|---|---:|---:|---:|---:|
| A -- Strict JSON | 0.00 | 0.00 | 0.00 | 0.00 |
| B -- Stream scanner | 0.00 | 100.00 | 0.00 | 33.33 |
| C -- Tolerant lexer | 100.00 | 100.00 | 13.33 | 66.67 |
| D -- Full system | 100.00 | 100.00 | 100.00 | 100.00 |

## Streaming

- Transport invariance, monolithic vs. one-character chunks: **100.00%**.
- Parser-side TTFPS for one-character chunks: median **0.3080 ms**, p95 **0.5281 ms**.
- TTFPS excludes model generation, network delay, rendering, and tool execution.

## Scope

These are deterministic parser micro-benchmarks. No claim about model-scale recovery, end-to-end first-pass execution, prompt tokens, constrained decoding, or provider-native latency is supported until real generation and external-baseline data are supplied.

## Repair-library payload baseline

`json_repair` exact SRR on Suites A and C payloads: **20.00%**. This payload-only result is not mixed into the cumulative protocol-layer ablation.
