# Auwgent CLI

Auwgent is a high-performance DSL and compiler for building agentic AI applications. This CLI allows you to compile `.agent` files into intermediate representation (IR) and generate type stubs for TypeScript and Python.

## Installation

You can install the Auwgent CLI globally via NPM:

```bash
npm install -g @snrraptopack/auwgent-cli
```

## Commands

### `generate`
The primary command for generating type stubs and IR from your agent files.

```bash
auwgent generate [PATH] [FLAGS]
```

- **PATH**: Optional. A file, directory, or glob pattern (e.g., `./src/**/*.agent`). If omitted, it reads from `auwgent.yml` or scans the current directory.
- **-t, --target <LANG>**: Target language (`ts`, `python`, or `both`). Defaults to `ts`.
- **-o, --output <DIR>**: Shared output directory for generated code.
- **-w, --watch**: Watch for changes and regenerate automatically.
- **-c, --config <FILE>**: Path to a custom config file (default: `auwgent.yml`).

### `compile`
Lower-level command to just produce the IR JSON without generating language-specific stubs.

```bash
auwgent compile [PATH] [FLAGS]
```

## Configuration (`auwgent.yml`)

You can create an `auwgent.yml` file in your project root to avoid passing flags every time.

```yaml
# auwgent.yml
source: "./agents"       # Source folder or glob
output: "./generated"    # Output folder
targets:                 # List of target languages
  - ts
  - python
```

## Watch Mode

Use the `--watch` (or `-w`) flag to start a persistent process that monitors your `.agent` files. Every time you save a change, Auwgent will instantly re-validate your syntax and update the generated code.

```bash
auwgent generate ./agents --watch
```

## IDE Support

For the best experience, install the **Auwgent VSCode Extension** to get syntax highlighting and real-time error diagnostics.
