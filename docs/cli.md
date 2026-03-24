# CLI Reference

## Invocation

```
chipmunk <job.yaml> [--output <file>]
chipmunk postprocessors
```

## Commands

### Generate NC Code

```bash
chipmunk job.yaml
```

Reads the YAML job file, generates toolpaths, runs the post-processor, and writes NC code to stdout. Errors and diagnostics go to stderr.

```bash
chipmunk job.yaml --output part.H
```

Same, but writes to a file instead of stdout. Prints the output path to stdout on success.

### List Post-Processors

```bash
chipmunk postprocessors
```

Lists all available post-processor IDs (built-in and user-defined).

## Options

| Flag | Short | Description |
|---|---|---|
| `--output <path>` | `-o` | Write NC output to file instead of stdout |

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Error — missing parameter, invalid YAML, post-processor not found, Lua error, etc. |

## Output Conventions

- **stdout**: NC code (when no `--output` is given). This means you can pipe directly to other tools.
- **stderr**: All errors, warnings, and diagnostic logging.

This separation is intentional — `chipmunk job.yaml | ftp ...` works because only NC code reaches the pipe.

## Build from Source

```bash
cargo build                  # Debug build
cargo build --release        # Release build
cargo test                   # Run tests
```

The binary is at `target/debug/chipmunk` or `target/release/chipmunk`.
