# Dependencies

## Runtime Dependencies

### clap

**What**: Command-line argument parser.
**Why**: Parses `chipmunk drill.yaml --output DRILL.H` into structured data. The `derive` feature lets you define CLI args as a Rust struct with attributes — clap generates the parser, help text, and validation from that.

### serde

**What**: Serialization/deserialization framework.
**Why**: The standard way to convert between Rust structs and data formats (YAML, JSON, etc.). The `derive` feature lets you add `#[derive(Serialize, Deserialize)]` to a struct and serde handles the rest. Nearly every Rust project that reads config files uses serde.

### serde_yaml

**What**: YAML backend for serde.
**Why**: Job files are YAML. This crate teaches serde how to read/write YAML specifically. You pair it with serde the same way you'd pair `serde_json` for JSON — serde is the framework, `serde_yaml` is the format driver.

### mlua

**What**: Lua scripting language bindings for Rust.
**Why**: Post-processors are Lua scripts. mlua embeds a Lua VM into the binary so we can pass NC data from Rust into Lua and get formatted NC code back. `lua55` selects Lua 5.5; `vendored` compiles Lua from source so there's no system dependency — it just works on any machine.

### tracing

**What**: Structured logging/diagnostics framework.
**Why**: For log messages (`tracing::info!`, `tracing::error!`). Unlike `println!`, tracing supports log levels, structured fields, and can be filtered at runtime (e.g. `RUST_LOG=debug`).

### tracing-subscriber

**What**: Log output backend for tracing.
**Why**: `tracing` defines the logging API but doesn't print anything by itself. `tracing-subscriber` is the piece that actually formats and writes log messages to the terminal. You initialize it once at startup.

### anyhow

**What**: Ergonomic error handling for applications.
**Why**: Lets you use `anyhow::Result` and the `?` operator to propagate errors without defining custom error types for every function. Ideal for application-level code (CLI, top-level orchestration). Automatically captures error chains and backtraces.

### thiserror

**What**: Derive macro for defining custom error types.
**Why**: For library-level errors where callers need to match on specific variants (e.g., `JobParseError::MissingField`). You define an enum, add `#[derive(thiserror::Error)]`, and it implements `std::error::Error` for you. Complements `anyhow` — use `thiserror` in library code, `anyhow` in application code.

## Dev Dependencies

### tempfile

**What**: Creates temporary files and directories that auto-delete.
**Why**: For integration tests that write output files (e.g., testing `--output DRILL.H`). The temp file cleans up after the test regardless of pass/fail.

### approx

**What**: Approximate floating-point comparison.
**Why**: Floating-point math means `0.1 + 0.2 != 0.3`. When testing toolpath coordinates, `approx` provides `assert_relative_eq!` so you can compare floats with a tolerance instead of exact equality.

## Deferred Dependencies (not in Cargo.toml yet)

These are listed in the design docs for later phases:

| Crate | Phase | Purpose |
|-------|-------|---------|
| `opencascade-rs` | 3+ | B-rep geometry kernel for STEP/STL import |
| `geo` | 4 | 2D geometry primitives for toolpath generators |
| `geo-clipper` | 4 | Polygon offset (Clipper2) for pocket/profile toolpaths |
| `nalgebra` | 3+ | Linear algebra, coordinate transforms |
| `serde_json` | 3+ | JSON for project files, tool library import/export |
| `uuid` | 3+ | Unique IDs for projects, operations |
| `chrono` | 3+ | Timestamps for project metadata |
| `axum`, `tokio`, `tower`, `tower-http` | backlog | REST API server |
