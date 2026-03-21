# CAMproject — Architecture Overview

## Purpose

A CLI-first CAM (Computer-Aided Manufacturing) tool that generates NC code for CNC milling machines. SVG or DXF files are used as input, machining operations are defined via YAML job files, and the tool exports controller-agnostic NC code through pluggable post-processors.

A REST API and browser frontend exist as a parallel interface for future use (remote access, GUI workflows) but the CLI is the primary interface.

## High-Level Architecture

```
  ┌─────────────────────┐         ┌──────────────────────┐
  │       CLI            │         │   API (axum)          │
  │  camproject mill     │         │   REST + WebSocket    │
  │  camproject drill    │         │   (future frontend,   │
  │  camproject serve    │         │    remote access)     │
  └──────────┬──────────┘         └──────────┬───────────┘
             │                               │
             │         call directly         │
             └──────────────┬────────────────┘
                            │
          ┌─────────────────▼─────────────────────┐
          │             Core Library               │
          │                                        │
          │  ┌─────────┐  ┌──────────┐  ┌──────┐  │
          │  │  core/  │  │toolpath/ │  │  nc/ │  │
          │  │ project │  │ facing   │  │  IR  │  │
          │  │ geometry│  │ profile  │  │ comp │  │
          │  │ tool    │  │ pocket   │  │      │  │
          │  │ operation│ │ drill    │  └──┬───┘  │
          │  └─────────┘  │ offset   │     │      │
          │               │ depth    │     │      │
          │  ┌─────────┐  └──────────┘     │      │
          │  │   io/   │              ┌────▼────┐  │
          │  │  svg    │              │  post-  │  │
          │  │  dxf    │              │processors│  │
          │  │  brep   │              │  (Lua)  │  │
          │  └─────────┘              └─────────┘  │
          └────────────────────────────────────────┘
```

The CLI and API are **peer-level thin shells** over the same library. The CLI calls library functions directly — no HTTP, no server process. The API does the same via HTTP handlers. Neither knows about the other.

## CLI and API Relationship

```
            core library functions
                    │
        ┌───────────┴───────────┐
        │                       │
  cli/mill.rs              api/routes.rs
  ┌──────────────┐         ┌──────────────┐
  │ parse args   │         │ parse request│
  │ call library │         │ call library │
  │ write files  │         │ return JSON  │
  └──────────────┘         └──────────────┘
```

Consequences:
- **CLI never goes through HTTP** — no server startup per invocation, no serialization overhead
- **API always calls real code** — same paths as the CLI, implicitly tested when CLI tests pass
- **No refactoring needed** when the frontend arrives — the API is already wired to working logic
- **Remote access works today** — `camproject serve` exposes the same operations over HTTP for scripting, CI, or a future frontend

## Data Flow

```
SVG/DXF + YAML job
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

Operations are selected by SVG stroke color. Each color maps to a full operation configuration in the YAML job file. Circles → drill points or circular pockets. Closed paths → profile, pocket, or facing area. One SVG + one YAML → one or more NC files (split by tool).

### 4. Controller agnosticism

Toolpaths are generated as abstract segment sequences (rapid, linear, arc). These are compiled to a controller-neutral IR (`NCBlock` list). Only the final Lua post-processor step formats machine-specific output. The same toolpath compiles to Heidenhain conversational, G-code, or any other format.

### 5. No-tool-changer workflow

Z=0 is set at the tool tip before each program run — no tool length measurement or compensation needed. Operations are grouped by tool and exported as separate NC files. Each file is self-contained: spindle on, moves, spindle off.

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

- **Browser frontend** — 2D canvas viewport, operation panels, NC preview
- **3D projects** — STEP/STL input, B-rep slicer, Three.js viewport
- **Inkscape extension** — Extensions > CAM menu; calls CLI or local API
- **Stock simulation**, **part update pipeline**, **CAD integrations**

See `tasks/backlog.md` for design notes on each.
