# Chipmunk — Architecture Overview

## Purpose

A CAM (Computer-Aided Manufacturing) kernel for generating NC code for CNC milling machines. The core library — geometry, toolpaths, NC IR, post-processing — is a pure computational engine with no framework dependencies. Interfaces (CLI, REST API, browser frontend) are thin adapters over this kernel.

Machining jobs are defined in YAML job files that reference SVG or DXF geometry; the kernel exports controller-agnostic NC code through pluggable post-processors.

The CLI is the primary and only interface for Phases 1–4. A REST API and browser frontend are deferred to the backlog — the kernel architecture keeps them decoupled so they can be added later without refactoring.

## High-Level Architecture

```
  ┌─────────────────────────────────┐
  │              CLI                │
  │  chipmunk [job.yaml]          │
  │  chipmunk postprocessors      │
  └──────────────┬──────────────────┘
                 │  calls directly
                 ▼
  ┌──────────────────────────────────────────┐
  │              Core Library                │
  │                                          │
  │  ┌─────────┐  ┌──────────┐  ┌────────┐  │
  │  │  core/  │  │toolpath/ │  │   nc/  │  │
  │  │ project │  │ facing   │  │   IR   │  │
  │  │ geometry│  │ profile  │  │  comp  │  │
  │  │ tool    │  │ pocket   │  │        │  │
  │  │operation│  │ drill    │  └───┬────┘  │
  │  └─────────┘  │ offset   │      │       │
  │               │ depth    │      │       │
  │  ┌─────────┐  └──────────┘      │       │
  │  │   io/   │              ┌─────▼────┐  │
  │  │  svg    │              │  post-   │  │
  │  │  dxf    │              │processors│  │
  │  │  brep   │              │  (Lua)   │  │
  │  └─────────┘              └──────────┘  │
  └──────────────────────────────────────────┘

  (REST API — deferred, see tasks/backlog.md)
```

The CLI calls library functions directly — no HTTP, no server process. When the REST API is built it will be a peer-level thin shell over the same library, not a wrapper around the CLI.

## Data Flow

```
YAML job (geometry: path/to/file.svg)
        │
        ▼
  io/: parse geometry, group by stroke color
        │
        ▼
  core/: Tool, Setup, Operation structs
        │
        ▼
  toolpath/: generate segments (rapid, linear, arc, drill point)
        │
        ▼
  nc/: compile to NCBlock IR (controller-neutral)
        │
        ▼
  nc/bridge: Lua VM → post-processor → NC string
        │
        ▼
  .H / .nc / .gcode file
```

Each stage is independently testable and produces a well-defined output type.

## Design Principles

### 1. CLI first, API as peer

The CLI is the primary interface during active development — it gives direct hardware feedback with no server overhead. The API is a peer consumer of the same library, not a wrapper around the CLI. Both are implemented; the CLI is what gets tested against real machines first.

### 2. Separation of concerns

`core/`, `toolpath/`, `nc/`, and `io/` are pure computational modules with no dependency on axum or clap. They are independently testable without a running server or argument parser. `cli/` and `api/` are thin adapters.

### 3. SVG color workflow

Operations are selected by SVG stroke color. Each color maps to a full operation configuration in the YAML job file, which also declares the geometry path (`geometry: part.svg`). Circles → drill points or circular pockets. Closed paths → profile, pocket, or facing area. One job YAML → NC output to stdout or `--output`.

The WCS origin is defined by a marker circle drawn in the SVG at a dedicated color declared in the YAML (`wcs_marker_color`). The importer reads the circle's center as the WCS (0, 0) and excludes it from operation geometry. If `wcs_marker_color` is not set, the WCS origin falls at the SVG coordinate origin (bottom-left corner after Y-axis correction).

A single-color SVG also works: the entire file is treated as one operation group, no color disambiguation needed.

### 4. Controller agnosticism

Toolpaths are generated as abstract segment sequences (rapid, linear, arc). These are compiled to a controller-neutral IR (`NCBlock` list). Only the final Lua post-processor step formats machine-specific output. The same toolpath compiles to Heidenhain conversational, G-code, or any other format.

### 5. No-tool-changer workflow

Z=0 is defined in WCS — which commonly coincides with the tool tip but does not have to. Tool length compensation is supported but not required. Operations can be combined in one program or split into one file per tool; per-tool export works well when loading tools manually without an ATC.

### 6. Trust the operator

Warnings only for physically impossible situations (tool wider than pocket, depth exceeds geometry). Aggressive feeds, deep cuts, and unconventional strategies are the operator's prerogative.

### 7. Plugin post-processors

Post-processors are Lua modules embedded at compile time (`include_str!`). User post-processors are `.lua` files placed in the config directory, discovered at startup. A fresh Lua VM is created per NC generation call — no shared state between runs.

## Technology Choices

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Performance-critical geometry and toolpath work; strong type system; single binary distribution |
| CLI parsing | clap | Ergonomic subcommand + flag parsing |
| API framework | axum | Async HTTP + WebSocket; same binary as CLI via subcommand |
| Serialization | serde + serde_json + serde_yaml | JSON for API/project files; YAML for job files |
| Geometry kernel | opencascade-rs | B-rep geometry, SVG/DXF import, exact curves |
| 2D geometry | geo + geo-clipper | Polygon offset via Clipper2 bindings |
| Linear algebra | nalgebra | Vectors, matrices, transforms |
| Post-processors | Lua 5.4 via mlua | Embedded VM (~300KB); designed for string formatting; safe sandboxed execution |
| HTTP client | reqwest | Async; used for CAD integrations (Onshape etc.) |

## Deferred (backlog)

- **REST API** — axum server, peer to the CLI, required before any frontend work
- **Turning** — lathe toolpaths, turning cycles, facing/profiling/threading
- **Browser frontend** — 2D canvas viewport, operation panels, NC preview
- **3D projects** — STEP/STL input, B-rep slicer, Three.js viewport
- **Inkscape extension** — Extensions > CAM menu; calls CLI or local API
- **Stock simulation**, **part update pipeline**, **CAD integrations**

See `tasks/backlog.md` for design notes on each.
