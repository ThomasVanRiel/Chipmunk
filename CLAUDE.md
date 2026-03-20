# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

CAMproject is a browser-based CAM (Computer-Aided Manufacturing) tool for generating NC code for CNC milling machines. It accepts 3D models (STL, STEP) and 2D drawings (DXF, SVG) as input, lets the user define machining operations, generates toolpaths, and exports controller-agnostic NC code through pluggable post-processors.

- **License**: MIT
- **Remote**: `git@github.com:ThomasVanRiel/CAMproject.git`

## Architecture

**Backend**: Rust (axum) — handles geometry processing, toolpath generation, NC IR compilation, API serving.
**Geometry kernel**: OpenCascade (via opencascade-rs) — B-rep is the primary geometry representation. Triangle meshes generated on demand for display only.
**Post-processors**: Python (via PyO3) — pluggable NC code formatters for different CNC controllers.
**Frontend**: TypeScript + Three.js — browser-based 3D viewport (3D projects) or 2D top-down view (2.5D projects).
Communication via REST API + WebSocket (for toolpath generation progress).

**Project types**: 3D (STEP/STL input, B-rep solids, 3D viewport) or 2.5D (DXF/SVG input, 2D wires/faces, top-down view). Set at project creation, immutable.

### Data Flow

```
File/CAD Import → PartGeometry → Operation (geometry + tool + params)
    → Toolpath (Rust) → NCBlock IR (Rust) → PostProcessor (Python via PyO3) → NC code string
```

### Module Dependency Rules (no circular deps)

```
api/            → core/, toolpath/, nc/, io/, integrations/
core/           → (no internal deps — only geo, nalgebra, serde)
toolpath/       → core/
nc/             → core/ (bridge.rs uses PyO3)
io/             → core/
integrations/   → io/, core/
utils/          → (no internal deps)
```

Key principle: `core/`, `toolpath/`, `nc/`, `io/` have **zero web framework dependencies**. They are pure computational Rust, independently testable.

### Three-Layer NC Generation

1. **Toolpath** — pure geometry segments (rapid/linear/arc with XYZ coordinates)
2. **NCBlock IR** — controller-neutral program (adds spindle, tools, coolant, compensation, cycles, optional skips)
3. **Post-processor** — formats to machine-specific output (G-code, Heidenhain conversational, Sinumerik)

Post-processors are written in Python, pluggable via entry_points, and invoked from Rust via PyO3. Built-in: LinuxCNC, Grbl, Marlin, Generic Fanuc, Sinumerik, Heidenhain TNC.

### Cutter Compensation

Operations support two modes: **CAM mode** (software computes offset toolpath) and **Controller mode** (emits G41/G42 or RL/RR, controller applies offset at runtime). Roughing typically uses CAM mode; finishing uses controller mode so the operator can fine-tune dimensions.

### Optional Operations

Operations can be marked optional with a skip level (1-9). Post-processors implement this via block delete (`/` prefix) or conditional jumps (controller-specific labels/variables), depending on the target machine.

### Canned Cycles

The IR supports `CycleDefine`/`CycleCall` blocks. Toolpath generators always produce explicit moves as fallback. Post-processors that declare cycle support emit native cycles (G81/G83 for G-code, CYCL DEF for Heidenhain); others use the expanded moves.

## Build & Development Commands

```bash
# Build the Rust backend
cargo build                          # Debug build
cargo build --release                # Release build

# Run the server
cargo run                            # Production: serves frontend from frontend/dist/
cargo run -- --dev --port 8000       # Development: API only, CORS enabled

# Rust tests
cargo test                           # All tests
cargo test test_pocket               # Tests matching keyword
cargo test -- --nocapture            # Show output
cargo clippy                         # Lint
cargo fmt                            # Format

# Post-processors (Python)
cd postprocessors
uv sync                              # Install deps
uv run pytest                        # Run tests
uv run ruff check src/               # Lint
uv run ruff format src/              # Format

# Frontend
cd frontend && npm install
cd frontend && npm run dev           # Vite dev server on :5173, proxies /api to :8000
cd frontend && npm run build         # Build to frontend/dist/
```

## Design Documents

Detailed design docs live in `docs/`:

| Doc | Contents |
|-----|----------|
| `00-overview.md` | Architecture, tech choices, design principles |
| `01-data-model.md` | Core types: Project, PartGeometry, Tool, Operation, Toolpath |
| `02-api-design.md` | REST API + WebSocket spec with request/response examples |
| `03-nc-and-postprocessors.md` | NCBlock IR (Rust), PostProcessor ABC (Python), PyO3 bridge, canned cycles, optional operations |
| `04-toolpath-algorithms.md` | Slicing, offset, facing, profile, pocket, drill algorithms |
| `05-frontend-design.md` | Three.js viewport, UI layout, panels, Vite build |
| `06-project-structure.md` | Directory tree, Cargo.toml, pyproject.toml (post-processors), dependency rules |
| `07-implementation-phases.md` | Phased task breakdown with deliverables |
| `08-integrations.md` | CAD integrations: Onshape API, FreeCAD, watch folder |
| `09-part-update.md` | Geometry change handling: diff, alignment, operation audit |

When implementing a feature, read the relevant design doc first. The docs are the source of truth for architectural decisions.

## Key Design Decisions

- **Rust backend + Python post-processors**: Rust for performance-critical computation (geometry, toolpaths, NC IR). Python for post-processors because they're the most likely extension point, and Python's string manipulation + entry_points system makes custom post-processors easy to write.
- **Trust the operator**: Only error on physically impossible geometry (tool wider than pocket, etc.). Never warn about aggressive feeds or deep cuts — that's the operator's call.
- **Auto-persistence**: Every change is saved immediately. No save button. Undo/redo via persistent command history (JSON patches). User can clear history if project file grows large.
- **Setup grouping**: Operations are grouped under setups. Each setup defines WCS, stock, and clearance height. Child operations inherit these with optional per-operation overrides. Full retraction between setups is handled by the post-processor.
- **Stock is optional** — the operator knows their stock. Stock definition is only needed later for optimization (avoid air cuts), simulation, and rest machining.
- **Tool numbers + machine field**: Tools have both `tool_number` (T1, T2) and `name`. Tool numbers are NOT unique within a project — uniqueness is scoped per `machine` value, supporting multi-machine workflows. Post-processor decides whether to call by number or name.
- **Tools carry recommended cutting data** (feed, speed, coolant) that auto-populates operations when selected. User can always override per-operation.
- **Tool library is global** (persistent across projects). Tools are copied into a project and can be edited per-project. Import/export via JSON.
- **Heidenhain is not a G-code dialect** — it's a completely different language (conversational format). The PostProcessor `generate()` method is designed to be fully overridable for this reason.
- **Part provenance** tracks where geometry came from (file path, Onshape doc ID, etc.) so it can be refreshed from source.
- **Part update pipeline** (diff → registration → change report → operation audit → user review) ensures CAM operations are preserved when the CAD model changes. Never silently break a project.
- **Post-processor plugin system** uses Python entry_points so third-party packages can register custom post-processors.
- **Face selection for orientation**: User clicks a face on the tessellated mesh → `face_ids[triangle_index]` maps to B-rep face → backend reads face normal, computes orientation transform. Z+ up, Z=0 at face surface.
- **Implementation order**: 2.5D projects first (DXF/SVG → toolpaths → NC code, Phases 1-3 = MVP). 3D projects (STEP/STL, Three.js viewport) layered on in Phase 5.

## Discussion Documents

Design discussions and decision records live in `discussions/`:

| Doc | Contents |
|-----|----------|
| `DECISIONS.md` | **Quick-reference index** of all decisions with links to source docs |
| `api.md` | API review — gaps, comparisons with other CAM tools, Thomas's decisions inline |
| `brep-geometry.md` | B-rep architecture — data model, import pipeline, toolpath impact, decisions table |
| `opencascade-bindings.md` | Assessment of opencascade-rs crate — coverage, gaps, build |
| `deferred-ideas.md` | Parked items — endpoint examples, face grouping details, setup sheets |
