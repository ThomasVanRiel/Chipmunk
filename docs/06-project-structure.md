# Project Structure & Build Configuration

## Directory Layout

```
CAMproject/
├── LICENSE                         # MIT License
├── CLAUDE.md                       # Claude Code guidance
├── Cargo.toml                      # Rust workspace root
├── Cargo.lock                      # Dependency lockfile
├── docs/                           # Design documentation (these files)
│   ├── 00-overview.md
│   ├── 01-data-model.md
│   ├── 02-api-design.md
│   ├── 03-nc-and-postprocessors.md
│   ├── 04-toolpath-algorithms.md
│   ├── 05-frontend-design.md
│   ├── 06-project-structure.md
│   ├── 07-implementation-phases.md
│   ├── 08-integrations.md
│   └── 09-part-update.md
├── src/
│   ├── main.rs                     # Entry point: starts axum server
│   ├── lib.rs                      # Library root, module declarations
│   ├── api/
│   │   ├── mod.rs                  # API module root, router setup
│   │   ├── routes.rs               # REST API endpoint handlers
│   │   ├── websocket.rs            # WebSocket handler for progress
│   │   └── state.rs                # AppState (shared server state)
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
│   │   ├── slicer.rs               # Mesh → 2D cross-sections
│   │   ├── offset.rs               # Polygon offset (clipper2 wrapper)
│   │   ├── facing.rs               # Facing toolpath generator
│   │   ├── profile.rs              # Profile toolpath generator
│   │   ├── pocket.rs               # Pocket toolpath generator
│   │   ├── drill.rs                # Drill cycle generator (Phase 4)
│   │   ├── ordering.rs             # Segment ordering optimization
│   │   └── depth_strategy.rs       # Multi-pass Z stepping
│   ├── nc/
│   │   ├── mod.rs
│   │   ├── ir.rs                   # NCBlock, BlockType enum
│   │   ├── compiler.rs             # Toolpath → NCBlock list
│   │   └── bridge.rs               # PyO3 bridge: NCBlock → Python objects
│   ├── io/
│   │   ├── mod.rs
│   │   ├── stl_reader.rs           # STL → PartGeometry
│   │   ├── dxf_reader.rs           # DXF → PartGeometry
│   │   ├── svg_reader.rs           # SVG → PartGeometry
│   │   ├── step_reader.rs          # STEP → PartGeometry (stub → Phase 5)
│   │   └── project_file.rs         # .camproj save/load (serde JSON)
│   ├── integrations/
│   │   ├── mod.rs
│   │   ├── onshape.rs              # Onshape REST API (Phase 4-5)
│   │   ├── freecad.rs              # FreeCAD file/CLI (Phase 5+)
│   │   └── watch_folder.rs         # Generic file watcher (Phase 3-4)
│   └── utils/
│       ├── mod.rs
│       └── math.rs                 # Arc fitting, geometric helpers
├── postprocessors/                 # Python package for post-processors
│   ├── pyproject.toml              # Python package config (post-processors only)
│   ├── src/
│   │   └── camproject_post/
│   │       ├── __init__.py
│   │       ├── base.py             # PostProcessor ABC, NCBlock Python types
│   │       ├── linuxcnc.py         # LinuxCNC post-processor
│   │       ├── grbl.py             # Grbl post-processor
│   │       ├── marlin.py           # Marlin post-processor
│   │       ├── generic_fanuc.py    # Generic Fanuc post-processor
│   │       ├── sinumerik.py        # Siemens Sinumerik
│   │       └── heidenhain.py       # Heidenhain TNC conversational
│   └── tests/
│       └── test_postprocessors.py  # Post-processor unit tests
├── frontend/
│   ├── package.json
│   ├── tsconfig.json
│   ├── vite.config.ts
│   ├── index.html
│   ├── src/
│   │   ├── main.ts
│   │   ├── api.ts
│   │   ├── types.ts
│   │   ├── viewport/
│   │   │   ├── scene.ts
│   │   │   ├── camera.ts
│   │   │   ├── mesh-loader.ts
│   │   │   ├── toolpath-renderer.ts
│   │   │   ├── stock-renderer.ts
│   │   │   └── grid.ts
│   │   ├── panels/
│   │   │   ├── operations.ts
│   │   │   ├── properties.ts
│   │   │   ├── tools.ts
│   │   │   └── nc-preview.ts
│   │   ├── dialogs/
│   │   │   ├── stock-setup.ts
│   │   │   └── tool-editor.ts
│   │   └── utils/
│   │       ├── dom.ts
│   │       └── format.ts
│   └── styles/
│       └── main.css
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
name = "camproject"
version = "0.1.0"
edition = "2024"
license = "MIT"
description = "Browser-based CAM tool for CNC milling NC code generation"
authors = ["Thomas Van Riel"]

[dependencies]
# Web framework
axum = { version = "0.8", features = ["ws", "multipart"] }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "fs", "trace"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Geometry
geo = "0.29"
geo-clipper = "0.8"                # Clipper2 bindings for polygon offset
nalgebra = "0.33"                  # Linear algebra, transforms
parry3d = "0.17"                   # 3D collision/geometry (mesh slicing)

# File I/O
stl_io = "0.7"                     # STL reading
dxf-rs = "0.6"                     # DXF reading
usvg = "0.44"                      # SVG parsing

# Python bridge (for post-processors)
pyo3 = { version = "0.23", features = ["auto-initialize"] }

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
thiserror = "2"

[dev-dependencies]
axum-test = "16"                   # HTTP testing for axum
tempfile = "3"
approx = "0.5"                     # Float comparison in tests

[features]
step = ["dep:opencascade-rs"]      # Optional STEP/OpenCascade support

[dependencies.opencascade-rs]
version = "0.2"
optional = true
```

## Post-Processor Python Package (pyproject.toml)

The `postprocessors/` directory is a standalone Python package. It contains the `PostProcessor` base class, NCBlock Python types, and all built-in post-processors.

```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "camproject-post"
version = "0.1.0"
description = "Post-processors for CAMproject CNC code generation"
license = "MIT"
requires-python = ">=3.11"
authors = [
    { name = "Thomas Van Riel" },
]
dependencies = []  # No external deps — NCBlock types come from Rust via PyO3

[project.optional-dependencies]
dev = [
    "pytest>=8.0",
    "ruff>=0.3",
]

[project.entry-points."camproject.postprocessors"]
linuxcnc = "camproject_post.linuxcnc:LinuxCNCPost"
grbl = "camproject_post.grbl:GrblPost"
marlin = "camproject_post.marlin:MarlinPost"
fanuc = "camproject_post.generic_fanuc:GenericFanucPost"
sinumerik = "camproject_post.sinumerik:SinumerikPost"
heidenhain = "camproject_post.heidenhain:HeidenhainPost"

[tool.ruff]
target-version = "py311"
line-length = 100
```

## Development Commands

```bash
# Build the Rust backend
cargo build                          # Debug build
cargo build --release                # Release build
cargo build --features step          # With STEP/OpenCascade support

# Run the server
cargo run                            # Production: serves frontend from frontend/dist/
cargo run -- --dev --port 8000       # Development: API only, CORS enabled

# Run tests
cargo test                           # All Rust tests
cargo test test_pocket               # Tests matching keyword
cargo test -- --nocapture            # Show println output
cargo test --test test_facing        # Single test file

# Lint & format
cargo clippy                         # Lint
cargo fmt                            # Format
cargo fmt -- --check                 # Check formatting without modifying

# Post-processor Python package
cd postprocessors
uv sync                              # Install Python deps
uv run pytest                        # Run post-processor tests
uv run ruff check src/               # Lint Python
uv run ruff format src/              # Format Python

# Frontend development
cd frontend
npm install
npm run dev                          # Vite dev server on :5173, proxies /api to :8000

# Frontend build (for production)
cd frontend
npm run build                        # Outputs to frontend/dist/
```

## Module Dependency Rules

```
api/  →  core/, toolpath/, nc/, io/, integrations/
          (API layer can import everything)

core/ →  (no internal dependencies, only external: geo, nalgebra, serde)

toolpath/ →  core/
              (toolpath generators use core types)

nc/   →  core/
          (NC compiler uses core types; bridge.rs uses PyO3)

io/   →  core/
          (readers produce core types)

integrations/ → io/, core/
                (integrations use readers and core types)

utils/ →  (no internal dependencies)
```

No circular dependencies. The `core/` module is the foundation that everything depends on but depends on nothing internal.

## PyO3 Bridge

The `nc/bridge.rs` module converts Rust `NCBlock` structs into Python objects that the post-processor Python code can work with. The bridge:

1. Initializes a Python interpreter (embedded via PyO3)
2. Discovers post-processors via `importlib.metadata.entry_points`
3. Converts `Vec<NCBlock>` → Python list of NCBlock objects
4. Calls the post-processor's `generate()` method
5. Returns the NC code string back to Rust

```
Rust: Vec<NCBlock>  →  PyO3  →  Python: list[NCBlock]  →  PostProcessor.generate()  →  str  →  PyO3  →  Rust: String
```

The `postprocessors/src/camproject_post/base.py` file defines the Python-side `NCBlock` dataclass and `PostProcessor` ABC that mirror the Rust types. PyO3 handles the type conversion automatically.
