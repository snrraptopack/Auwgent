# Real-generation input

Place consented/licensed model generations in JSONL and pass the file with
`--real-world`. Each row uses the same schema as `results/corpus.jsonl`:

```json
{"id":"qwen-001","suite":"D","category":"bfcl_simple","payload":"name: Alice","stream":"[tool_call: bench]\nname: Alice\n[/tool_call]","expected_args":{"name":"Alice"},"schema":{"name":{"type":"string","optional":false}},"expect_rejection":false,"model":"Qwen-2.5-7B-Instruct","model_scale":"7B"}
```

The harness does not call paid APIs. Generation collection must record model
revision, inference server/provider, decoding parameters, prompt/template,
dataset item ID, timestamp, and random seed in a companion manifest. Do not
label imported results as BFCL unless the prompt translation and scoring
protocol are documented and the dataset license permits redistribution.

