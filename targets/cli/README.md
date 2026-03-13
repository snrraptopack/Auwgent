# @snrraptopack/auwgent-cli

NPM wrapper for the native Auwgent CLI tools.

## Usage

```bash
# Install globally
npm install -g @snrraptopack/auwgent-cli

# Run the compiler
auwgent generate my_agent.agent
```

This package automatically downloads the high-performance native Rust binaries for your specific operating system (Windows, macOS, or Linux) during installation.

For full documentation on commands and configuration, see the [main README](https://github.com/snrraptopack/Auwgent).

## Release Tags

Release workflows are separated by tag prefix:

- `v*` -> CLI binaries + `@snrraptopack/auwgent-cli`
- `ts-v*` -> TypeScript SDK publish flow
- `py-v*` -> Python SDK publish flow

This keeps version bumps scoped to only the package being released.

## Local Smoke Test

Before publishing CLI changes, validate the npm tarball includes required files:

```bash
npm run verify:local-package
```
