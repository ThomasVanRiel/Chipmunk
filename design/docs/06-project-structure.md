# Project Structure & Build Configuration

## Directory Layout

```
chipmunk/
├── LICENSE                         # MIT License
├── CLAUDE.md                       # Claude Code guidance
├── Cargo.toml                      # Rust workspace root
├── Cargo.lock                      # Dependency lockfile
├── docs/                           # Design documentation (these files)
│   ├── 00-overview.md
│   ├── 01-data-model.md
│   ├── 03-nc-and-postprocessors.md
│   ├── 04-toolpath-algorithms.md
│   ├── 06-project-structure.md
│   ├── 07-implementation-phases.md
│   ├── 11-plugin-system.md
│   └── deferred/                   # Backlog — not needed for Phases 1–4
│       ├── 02-api-design.md
│       ├── 05-frontend-design.md
│       ├── 08-integrations.md
│       ├── 09-part-update.md
│       └── 10-opencascade.md
├── src/
│   ├── bin/
│   │   ├── chipmunk.rs           # CLI entry point: positional YAML + override flags
│   │   └── chipmunk_server.rs    # Server entry point (DEFERRED — see tasks/backlog.md)
│   ├── lib.rs                      # Library root, module declarations
│   ├── cli/
│   │   ├── mod.rs                  # CLI module root
│   │   └── postprocessors.rs       # `chipmunk postprocessors` subcommand handler
│   ├── api/                        # DEFERRED — add when REST API is implemented
│   ├── core/
│   │   ├── mod.rs
│   │   ├── project.rs              # Project container, save/load
│   │   ├── geometry.rs             # PartGeometry, StockDefinition, BoundingBox
│   │   ├── tool.rs                 # Tool definitions, ToolLibrary
│   │   ├── operation.rs            # Operation types and params
│   │   ├── toolpath.rs             # Toolpath, ToolpathSegment, MoveType
│   │   └── units.rs                # mm/inch enum and conversion
│   ├── toolpath/
│   │   ├── mod.rs
│   │   ├── registry.rs             # OperationType trait + compile-time type registry
│   │   ├── slicer.rs               # B-rep section at Z → exact 2D curves
│   │   ├── offset.rs               # Polygon offset (clipper2 wrapper)
│   │   ├── facing.rs               # Facing (implements OperationType)
│   │   ├── profile.rs              # Profile (implements OperationType)
│   │   ├── pocket.rs               # Pocket (implements OperationType)
│   │   ├── drill.rs                # Drill cycles (implements OperationType, Phase 4)
│   │   ├── ordering.rs             # Segment ordering optimization
│   │   └── depth_strategy.rs       # Multi-pass Z stepping
│   ├── nc/
│   │   ├── mod.rs
│   │   ├── ir.rs                   # NCBlock, BlockType enum
│   │   ├── compiler.rs             # Toolpath → NCBlock list
│   │   ├── bridge.rs               # mlua bridge: NCBlock → Lua tables → NC string
│   │   └── postprocessors/         # Built-in Lua post-processors (include_str! embedded)
│   │       └── mod.rs              # BUILTIN_POSTPROCESSORS registry
│   ├── io/
│   │   ├── mod.rs
│   │   ├── step_reader.rs          # STEP → TopoDS_Shape (via OpenCascade)
│   │   ├── stl_reader.rs           # STL → TopoDS_Shape (via OpenCascade sewing)
│   │   ├── dxf_reader.rs           # DXF → TopoDS_Wire/Face (2.5D projects)
│   │   ├── svg_reader.rs           # SVG → TopoDS_Wire/Face (2.5D projects)
│   │   ├── brep_io.rs              # Read/write .brep files (shape persistence)
│   │   └── project_file.rs         # .camproj save/load (serde JSON)
│   ├── integrations/
│   │   ├── mod.rs
│   │   ├── onshape.rs              # Onshape REST API (Phase 4-5)
│   │   ├── freecad.rs              # FreeCAD file/CLI (Phase 5+)
│   │   └── watch_folder.rs         # Generic file watcher (Phase 3-4)
│   └── utils/
│       ├── mod.rs
│       └── math.rs                 # Arc fitting, geometric helpers
├── postprocessors/                 # Lua post-processors (embedded at compile time)
│   ├── base.lua                    # Shared helpers (coord formatting, number formatting)
│   ├── heidenhain.lua              # Heidenhain TNC conversational (primary)
│   └── haas.lua                    # Haas (G-code example — starting point for other controllers)
├── frontend/                       # DEFERRED — not built yet (see tasks/backlog.md)
└── tests/
    ├── common/
    │   └── mod.rs                  # Shared test utilities, fixtures
    ├── test_geometry.rs
    ├── test_slicer.rs
    ├── test_offset.rs
    ├── test_facing.rs
    ├── test_profile.rs
    ├── test_pocket.rs
    ├── test_nc_compiler.rs
    ├── test_dxf_reader.rs
    ├── test_svg_reader.rs
    ├── test_api.rs
    └── fixtures/
        ├── cube.stl
        ├── simple_pocket.stl
        ├── rectangle.dxf
        └── circle.svg
```

## Cargo.toml

```toml
[package]
name = "chipmunk"
version = "0.1.0"
edition = "2024"
license = "MIT"
description = "CLI-first CAM tool for CNC milling NC code generation"
authors = ["Thomas Van Riel"]

[[bin]]
name = "chipmunk"
path = "src/bin/chipmunk.rs"

[[bin]]
name = "chipmunk-server"
path = "src/bin/chipmunk_server.rs"
required-features = ["server"]   # only built with --features server

[features]
server = ["axum", "tokio", "tower", "tower-http"]

[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Web framework (server feature only — not linked into chipmunk CLI binary)
axum = { version = "0.8", features = ["ws", "multipart"], optional = true }
tokio = { version = "1", features = ["full"], optional = true }
tower = { version = "0.5", optional = true }
tower-http = { version = "0.6", features = ["cors", "fs", "trace"], optional = true }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# Geometry
opencascade-rs = "0.2"             # B-rep geometry kernel (required)
geo = "0.29"                       # 2D geometry for polygon offset in toolpath generators
geo-clipper = "0.8"                # Clipper2 bindings for polygon offset
nalgebra = "0.33"                  # Linear algebra, transforms

# Lua bridge (for post-processors)
mlua = { version = "0.10", features = ["lua54", "vendored"] }

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
approx = "0.5"                     # Float comparison in tests

# No feature flags for geometry — OpenCascade is always required
```

## Post-Processor Lua Files

The `postprocessors/` directory contains the Lua source files embedded into the binary at compile time via `include_str!()`. No package manager or build step is needed — they are plain `.lua` files.

```
postprocessors/
├── base.lua         # Shared helpers: coord/number formatting, modal state tracking
├── heidenhain.lua   # Primary built-in
└── haas.lua         # Example G-code post-processor (starting point for other controllers)
```

Testing post-processors during development: the `tests/test_postprocessors.rs` integration test loads each Lua file via `mlua` and runs it against a fixed `Vec<NCBlock>` fixture, comparing the output against a golden file in `tests/fixtures/nc/`.

## Development Commands

```bash
# Build
cargo build                          # Debug build
cargo build --release                # Release build

# CLI
cargo run -- job.yaml --output part.H
cargo run -- job.yaml --check
cargo run -- postprocessors          # List available post-processors

# Server (deferred — requires --features server)
cargo run --bin chipmunk-server --features server
cargo run --bin chipmunk-server --features server -- --dev --port 8000

# Tests
cargo test                           # All Rust tests
cargo test test_pocket               # Tests matching keyword
cargo test -- --nocapture            # Show println output
cargo test --test test_facing        # Single test file

# Lint & format
cargo clippy
cargo fmt
cargo fmt -- --check
```

## Module Dependency Rules

```
cli/  →  core/, toolpath/, nc/, io/
          (CLI handlers call library functions directly — no HTTP)

api/  →  core/, toolpath/, nc/, io/, integrations/
          (deferred — same pattern as CLI when implemented)

core/ →  (no internal dependencies, only external: opencascade-rs, geo, nalgebra, serde)

toolpath/ →  core/
              (toolpath generators use core types)

nc/   →  core/
          (NC compiler uses core types; bridge.rs uses mlua)

io/   →  core/
          (readers produce core types)

integrations/ → io/, core/
                (integrations use readers and core types)

utils/ →  (no internal dependencies)
```

No circular dependencies. `core/`, `toolpath/`, `nc/`, `io/` have **zero framework dependencies** — no axum, no clap. `cli/` and `api/` are peer-level thin adapters.

## mlua Bridge

The `nc/bridge.rs` module converts `Vec<NCBlock>` to Lua tables and runs the post-processor:

1. Creates a fresh `mlua::Lua` instance (no shared state between calls)
2. Loads `base.lua` helper functions
3. Loads the selected post-processor Lua module (built-in or from config directory)
4. Reads `PostProcessorCapabilities` from the module fields before compilation
5. Converts `Vec<NCBlock>` → Lua table
6. Calls `M.generate(blocks, context)` → returns NC code string

```
Rust: Vec<NCBlock>  →  mlua  →  Lua tables  →  M.generate()  →  string  →  mlua  →  Rust: String
```

Built-in post-processors are registered in `nc/postprocessors/mod.rs` as `BUILTIN_POSTPROCESSORS: &[(&str, &str)]` = `[(name, lua_source)]` pairs, embedded via `include_str!()`. User post-processors are `.lua` files in the OS config directory, discovered at startup.
