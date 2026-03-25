# auwgent-sdk (Python)

The official Python SDK for [Auwgent](https://github.com/snrraptopack/Auwgent) — a compiler-first framework for building production-grade AI agents.

## Installation

```bash
pip install auwgent-sdk
```

## Requirements

- Python 3.8 or later
- The [Auwgent CLI](https://www.npmjs.com/package/@snrraptopack/auwgent-cli) to compile your agent definitions

## Usage

Compile your agent definition first:

```bash
auwgent generate
```

This produces a `generated/` folder containing the compiled IR and Python bindings. Then:

```python
import asyncio
from auwgent import create_auwgent
from generated.main_agent_types import ir

agent = create_auwgent(ir, {
    "apiKeys": {
        "geminiApiKey": "YOUR_API_KEY"
    }
})

async def handle_intent(name: str, value: dict, agent_name: str):
    if name == "response_text":
        print(value.get("text"))

agent.on_intent(handle_intent)

asyncio.run(agent.run("Hello!"))
```

## Documentation

Full documentation is available at [auwgent.dev](https://auwgent.dev).

## License

MIT
