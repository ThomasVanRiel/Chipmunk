# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

CAMproject is a CLI-first CAM (Computer-Aided Manufacturing) tool for generating NC code for CNC milling machines. SVG or DXF files are used as input geometry; machining operations are defined in YAML job files; NC code is exported through pluggable Lua post-processors. A REST API exists as a peer interface for future use (frontend, remote access) but the CLI is the primary interface.

- **License**: MIT
- **Remote**: `git@github.com:ThomasVanRiel/CAMproject.git`

## Ground Rules for Claude

**Do not write or modify source code unless explicitly asked.**

Claude is a useful assistant for research, documentation, planning, and answering questions about the codebase. The source code is human-authored by design. Helping with docs, auditing for inconsistencies, drafting phase plans, or explaining design decisions is welcome. Opening a `.rs`, `.lua`, or `.toml` source file and editing it is not — unless the user has explicitly asked for that specific change.

When in doubt: document, don't code.

---

## Implementation Status

**Pre-implementation** — only design documentation exists. No source code has been written yet. Phase 1 (backend scaffolding + SVG/DXF import) is the starting point. See `docs/07-implementation-phases.md` for the full phased breakdown.

All `src/`, `postprocessors/`, and `frontend/` paths described below are planned structure, not yet on disk.

## Architecture

**CLI**: Primary interface — `camproject mill`, `camproject drill`, `camproject postprocessors`. Calls the core library directly; no HTTP overhead.
**API**: Deferred. Will be a peer interface (axum) over the same library functions — not a wrapper around the CLI. Needed before any frontend work. See `tasks/backlog.md`.
**Geometry kernel**: OpenCascade (via opencascade-rs) — B-rep for exact curves; SVG and DXF import.
**Post-processors**: Lua (via mlua) — pluggable NC code formatters. ~300KB VM embedded at compile time. Built-ins via `include_str!`; user post-processors are `.lua` files in the config directory.
**Frontend**: Deferred. See `tasks/backlog.md`.

### Data Flow

```
SVG/DXF (grouped by stroke color) + YAML job
    → io/: geometry per color group
    → core/: Tool, Setup, Operation
    → toolpath/: segments (rapid, linear, arc, drill point)
    → nc/: NCBlock IR (controller-neutral)
    → nc/bridge: Lua post-processor → NC string
    → .H / .nc / .gcode file
```

### Module Dependency Rules (no circular deps)

```
cli/            → core/, toolpath/, nc/, io/
api/            → core/, toolpath/, nc/, io/, integrations/
core/           → (no internal deps — only geo, nalgebra, serde)
toolpath/       → core/
nc/             → core/
io/             → core/
integrations/   → io/, core/
utils/          → (no internal deps)
```

Key principle: `core/`, `toolpath/`, `nc/`, `io/` have **zero framework dependencies** — no axum, no clap. They are pure computational Rust, independently testable. `cli/` and `api/` are peer-level thin adapters over the same library.

### Three-Layer NC Generation

1. **Toolpath** — pure geometry segments (rapid/linear/arc with XYZ coordinates)
2. **NCBlock IR** — controller-neutral program (adds spindle, tools, coolant, compensation, cycles, optional skips)
3. **Post-processor** — formats to machine-specific output (G-code, Heidenhain conversational, Sinumerik)

Post-processors are Lua modules (~300KB VM) embedded at compile time via `include_str!()`. A fresh Lua VM is created per NC generation call. Built-in post-processors: Heidenhain TNC (primary), Haas (example). User post-processors are `.lua` files in the config directory, discovered at startup.

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

# CLI subcommands
cargo run -- serve                   # Start web server (production)
cargo run -- serve --dev --port 8000 # API only, CORS enabled (development)
cargo run -- drill holes.dxf --postprocessor heidenhain --output DRILL.H
cargo run -- drill --at 25,15 --at 75,15 --postprocessor heidenhain
cargo run -- postprocessors          # List available post-processors

# Rust tests
cargo test                           # All tests
cargo test test_pocket               # Tests matching keyword
cargo test -- --nocapture            # Show output
cargo clippy                         # Lint
cargo fmt                            # Format

```

## Design Documents

Detailed design docs live in `docs/`:

| Doc | Contents |
|-----|----------|
| `00-overview.md` | Architecture, tech choices, design principles |
| `01-data-model.md` | Core types: Project, PartGeometry, Tool, Operation, Toolpath |
| `02-api-design.md` | REST API + WebSocket spec **(DEFERRED — see tasks/backlog.md)** |
| `03-nc-and-postprocessors.md` | NCBlock IR (Rust), mlua bridge, Lua post-processor API, drill strategies, canned cycles, optional operations |
| `04-toolpath-algorithms.md` | Slicing, offset, facing, profile, pocket, drill algorithms |
| `05-frontend-design.md` | Three.js viewport, UI layout, panels, Vite build **(DEFERRED — see tasks/backlog.md)** |
| `06-project-structure.md` | Directory tree, Cargo.toml, dependency rules |
| `07-implementation-phases.md` | Phased task breakdown with deliverables |
| `08-integrations.md` | CAD integrations: Onshape API, FreeCAD, watch folder |
| `09-part-update.md` | Geometry change handling: diff, alignment, operation audit |
| `10-opencascade.md` | OCCT API usage by module, custom cxx.rs bindings, thread safety, error handling |
| `11-plugin-system.md` | Post-processor Lua plugin mechanics (registry, mlua bridge, testing); toolpath operation Rust trait system |

When implementing a feature, read the relevant design doc first. The docs are the source of truth for architectural decisions.

## Key Design Decisions

- **Rust backend + Lua post-processors**: Rust for performance-critical computation (geometry, toolpaths, NC IR). Lua for post-processors because they're the most likely extension point, Lua is designed for embedding, and post-processors are fundamentally string formatters that need no heavy runtime. The entire Lua VM adds ~300KB vs ~50MB for Python. Toolpath operation plugins use Rust traits (compiled in) since geometry code must be fast.
- **Trust the operator**: Only error on physically impossible geometry (tool wider than pocket, etc.). Never warn about aggressive feeds or deep cuts — that's the operator's call.
- **Never infer**: If a required parameter is missing or a tool ID cannot be resolved, exit with a hard error to stderr (exit code 1). Never silently fill in defaults, guess values, or infer intent. The user asked the impossible — tell them clearly.
- **Auto-persistence**: Every change is saved immediately. No save button. Undo/redo via persistent command history (JSON patches). User can clear history if project file grows large.
- **Setup grouping**: Operations are grouped under setups. Each setup defines WCS, stock, and clearance height. Child operations inherit these with optional per-operation overrides. Full retraction between setups is handled by the post-processor.
- **Stock is optional** — the operator knows their stock. Stock definition is only needed later for optimization (avoid air cuts), simulation, and rest machining.
- **Tool numbers + machine field**: Tools have both `tool_number` (T1, T2) and `name`. Tool numbers are NOT unique within a project — uniqueness is scoped per `machine` value, supporting multi-machine workflows. Post-processor decides whether to call by number or name.
- **Tools carry recommended cutting data** (feed, speed, coolant) that auto-populates operations when selected. User can always override per-operation.
- **Tool library is global** (persistent across projects). Tools are copied into a project and can be edited per-project. Import/export via JSON.
- **Heidenhain is not a G-code dialect** — it's a completely different language (conversational format). The PostProcessor `generate()` method is designed to be fully overridable for this reason.
- **Part provenance** tracks where geometry came from (file path, Onshape doc ID, etc.) so it can be refreshed from source.
- **Part update pipeline** (diff → registration → change report → operation audit → user review) ensures CAM operations are preserved when the CAD model changes. Never silently break a project.
- **Post-processor plugin system** uses Lua config directory: built-ins embedded at compile time via `include_str!`; user post-processors are `.lua` files in the config directory discovered at startup.
- **Face selection for orientation**: User clicks a face on the tessellated mesh → `face_ids[triangle_index]` maps to B-rep face → backend reads face normal, computes orientation transform. Z+ up, Z=0 at face surface.
- **Implementation order**: CLI first. Phase 1 = scaffolding + import. Phase 2 = manual drill CLI + Heidenhain hardware test. Phase 3 = automatic drill cycles + per-tool export. Phase 4 = 2.5D milling (SVG color workflow + YAML job file). Frontend and 3D projects are deferred to backlog.

## Discussion Documents

Design discussions and decision records live in `discussions/`:

| Doc | Contents |
|-----|----------|
| `DECISIONS.md` | **Quick-reference index** of all decisions with links to source docs |
| `api.md` | API review — gaps, comparisons with other CAM tools, Thomas's decisions inline |
| `brep-geometry.md` | B-rep architecture — data model, import pipeline, toolpath impact, decisions table |
| `opencascade-bindings.md` | Assessment of opencascade-rs crate — coverage, gaps, build |
| `deferred-ideas.md` | Parked items — endpoint examples, face grouping details, setup sheets |
