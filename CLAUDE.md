# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Chipmunk is a CLI-first CAM (Computer-Aided Manufacturing) tool for generating NC code for CNC milling machines. Machining jobs are defined in YAML job files that reference SVG or DXF geometry; NC code is exported through pluggable Lua post-processors. A REST API exists as a peer interface for future use (frontend, remote access) but the CLI is the primary interface.

- **License**: MIT
- **Remote**: `git@github.com:ThomasVanRiel/chipmunk.git`

## Ground Rules for Claude

**Do not write or modify source code unless explicitly asked.**

Claude is a useful assistant for research, documentation, planning, and answering questions about the codebase. The source code is human-authored by design. Helping with docs, auditing for inconsistencies, drafting phase plans, or explaining design decisions is welcome. Opening a `.rs`, `.lua`, or `.toml` source file and editing it is not — unless the user has explicitly asked for that specific change.

When in doubt: document, don't code.

---

## Implementation Status

**Active implementation** — Phase 1 is underway. Core scaffolding, operation type system, and YAML parsing are in place. See `DESIGN.md` for the full phased breakdown.

## Architecture

**CLI**: Primary interface — `chipmunk job.yaml`, `chipmunk postprocessors`. The YAML file is the sole input for NC generation. Calls the core library directly; no HTTP overhead.
**API**: Deferred. Will be a peer interface (axum) over the same library functions — not a wrapper around the CLI. Needed before any frontend work. See `design/tasks/backlog.md`.
**Geometry kernel**: OpenCascade (via opencascade-rs) — B-rep for exact curves; SVG and DXF import.
**Post-processors**: Lua (via mlua) — pluggable NC code formatters. ~300KB VM embedded at compile time. Built-ins via `include_str!`; user post-processors are `.lua` files in the config directory.
**Frontend**: Deferred. See `design/tasks/backlog.md`.

### Data Flow

```
YAML job (geometry: path/to/file.svg)
    → io/parsing: JobConfig → OperationConfig → Operation  (YAML-specific adapter)
    → io/geometry: parse SVG/DXF, group entities by stroke color
    → operations/: Operation (OperationCommon + OperationVariant + OperationType trait)
    → toolpath/: segments (rapid, linear, arc, drill point)
    → nc/: NCBlock IR (controller-neutral)
    → nc/bridge: Lua post-processor → NC string
    → .H / .nc / .gcode file
```

`OperationConfig` is a YAML-specific type in `io/`. It is not part of the core operation type system. Other IO surfaces (REST API, bindings) construct `Operation` directly — each IO layer owns its own config-to-operation conversion.

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
cargo run --bin chipmunk-server --features server                   # Start REST API server
cargo run --bin chipmunk-server --features server -- --dev --port 8000 # CORS enabled (development)
cargo run -- drill.yaml --output DRILL.H
cargo run -- job.yaml --output part.H
cargo run -- postprocessors          # List available post-processors

# Rust tests
cargo test                           # All tests
cargo test test_pocket               # Tests matching keyword
cargo test -- --nocapture            # Show output
cargo clippy                         # Lint
cargo fmt                            # Format

```

## Design Documents

Detailed design docs live in `design/docs/`:

| Doc | Contents |
|-----|----------|
| `00-overview.md` | Architecture, tech choices, design principles |
| `01-data-model.md` | Core types: Project, PartGeometry, Tool, Operation, Toolpath |
| `03-nc-and-postprocessors.md` | NCBlock IR (Rust), mlua bridge, Lua post-processor API, drill strategies, canned cycles, optional operations |
| `04-toolpath-algorithms.md` | Slicing, offset, facing, profile, pocket, drill algorithms |
| `06-project-structure.md` | Directory tree, Cargo.toml, dependency rules |
| `07-implementation-phases.md` | Phased task breakdown with deliverables |
| `11-plugin-system.md` | Post-processor Lua plugin mechanics (registry, mlua bridge, testing); toolpath operation Rust trait system |

Deferred docs (not needed for Phases 1–4) live in `design/docs/deferred/`:

| Doc | Contents |
|-----|----------|
| `02-api-design.md` | REST API + WebSocket spec |
| `05-frontend-design.md` | Three.js viewport, UI layout, panels, Vite build |
| `08-integrations.md` | CAD integrations: Onshape API, FreeCAD, watch folder |
| `09-part-update.md` | Geometry change handling: diff, alignment, operation audit |
| `10-opencascade.md` | OCCT API usage by module, custom cxx.rs bindings, thread safety, error handling |

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
- **Implementation order**: CLI first. Phase 1 = scaffolding + import. Phase 2 = manual drill (YAML, `strategy: manual`) + Heidenhain hardware test. Phase 3 = automatic drill cycles. Phase 4 = 2.5D milling (SVG color workflow, geometry embedded in YAML). No subcommands for NC generation — the binary takes a YAML file directly. Frontend and 3D projects are deferred to backlog.

## Discussion Documents

Design discussions and decision records live in `design/discussions/`:

| Doc | Contents |
|-----|----------|
| `DECISIONS.md` | **Quick-reference index** of all decisions with links to source docs |
| `api.md` | API review — gaps, comparisons with other CAM tools, Thomas's decisions inline |
| `brep-geometry.md` | B-rep architecture — data model, import pipeline, toolpath impact, decisions table |
| `opencascade-bindings.md` | Assessment of opencascade-rs crate — coverage, gaps, build |
| `deferred-ideas.md` | Parked items — endpoint examples, face grouping details, setup sheets |
