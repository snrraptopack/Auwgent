# Local Testing Guide

This repo is already configured so compile and generate read agent files from the manual-testing folder.

## Why This Works

The CLI config is in auwgent-compiler/auwgent.yml and contains:

- source: ../manual-testing/**/*.agent
- output: ../manual-testing/generated

That source pattern includes manual-testing/main.agent.

## Simple Commands

Run from the auwgent-compiler directory:

```powershell
cargo run -p auwgent-cli -- compile
cargo run -p auwgent-cli -- generate
```

Both commands use auwgent.yml automatically when no path is passed.

## What Gets Tested

- compile: parses, checks, and lowers .agent files to IR JSON.
- generate: parses, checks, lowers, and generates target stubs (TypeScript by default from config).

## Output Location

Generated outputs are written to:

- manual-testing/generated

## Optional Direct Target

If you only want one file, pass it explicitly:

```powershell
cargo run -p auwgent-cli -- compile ../manual-testing/main.agent
cargo run -p auwgent-cli -- generate ../manual-testing/main.agent
```
