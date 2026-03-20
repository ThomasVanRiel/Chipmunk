# CAMproject — Architecture Overview

## Purpose

A browser-based CAM (Computer-Aided Manufacturing) tool that generates NC code for CNC milling machines. The system accepts 3D models and 2D drawings as input, allows the user to define machining operations, generates toolpaths, and exports controller-agnostic NC code through pluggable post-processors.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Browser (Frontend)                   │
│  ┌──────────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐  │
│  │  Three.js 3D │ │Operations│ │   Tool   │ │   NC   │  │
│  │   Viewport   │ │  Panel   │ │ Library  │ │Preview │  │
│  └──────────────┘ └──────────┘ └──────────┘ └────────┘  │
└──────────────────────┬──────────────────────────────────┘
                       │ REST API + WebSocket
┌──────────────────────┴──────────────────────────────────┐
│                  Axum Backend (Rust)                     │
│                                                         │
│  ┌─────────┐  ┌──────────┐  ┌────┐  ┌───────────────┐   │
│  │  api/   │→ │  core/   │→ │ nc/│→ │postprocessors/│   │
│  │ routes  │  │ geometry │  │ IR │  │  (Python via  │   │
│  │ websock │  │ tools    │  │comp│  │   PyO3)       │   │
│  └─────────┘  │ ops      │  └────┘  │  linuxcnc     │   │
│               └──────────┘          │  grbl          │   │
│                    ↑                │  fanuc         │   │
│  ┌─────────┐  ┌──────────┐          │  heidenhain    │   │
│  │   io/   │  │toolpath/ │          └───────────────┘   │
│  │ step    │  │ slicer   │                              │
│  │ dxf     │  │ offset   │                              │
│  │ stl     │  │ facing   │                              │
│  │ svg     │  │ profile  │                              │
│  └─────────┘  │ pocket   │                              │
│               └──────────┘                              │
└─────────────────────────────────────────────────────────┘
```

## Hybrid Rust + Python Architecture

The project uses a **Rust backend** for all performance-critical computation (geometry processing, toolpath generation, NC IR compilation, API serving) and **Python for post-processors** to maximize extensibility.

**Why Rust for the backend?**
- Computational geometry and toolpath generation are CPU-intensive — Rust's performance is a natural fit
- Strong type system catches data model errors at compile time
- Axum provides async HTTP + WebSocket with excellent performance
- Memory safety without GC overhead

**Why Python for post-processors?**
- Post-processors are the most likely extension point for end users
- Python is widely known in the CNC/manufacturing community
- Custom post-processors often involve string formatting and controller-specific quirks — Python excels at this
- Python entry_points provide a mature plugin discovery mechanism
- PyO3 bridges Rust ↔ Python efficiently

**The boundary**: Rust produces `NCBlock` IR (a list of structured blocks). Python post-processors receive this IR as Python objects (via PyO3) and format it into machine-specific NC code strings. The Rust side never generates G-code directly.

## Design Principles

### 1. Separation of Concerns

`core/`, `toolpath/`, `nc/`, and `io/` are pure computational Rust modules with no web framework dependencies. They are independently testable without a running server or browser. The `api/` module is a thin adapter between HTTP and the core logic.

### 2. Data Flow is Unidirectional

```
File Import → PartGeometry → Operation (geometry + tool + params)
    → Toolpath → NCBlock list → PostProcessor (Python) → NC code string
```

Each stage is independently testable and produces a well-defined output.

### 3. Controller Agnosticism

Toolpaths are generated as abstract segment sequences (rapid, linear, arc). These are compiled to a controller-neutral intermediate representation (`NCBlock` list). Only the final post-processor step formats machine-specific output.

### 4. Plugin Extensibility

New post-processors are written in Python, registered via entry points, and discovered at runtime. New operation types, importers, and toolpath strategies are added as Rust modules.

### 5. No Singleton State

The `Project` struct is the root of all state. The server could theoretically serve multiple projects. All computation functions are pure (inputs → outputs).

### 6. Trust the Operator

The CAM tool is not a nanny. Warnings are shown only when something is **physically impossible** (e.g., tool wider than pocket, depth exceeds part geometry). Suboptimal parameters (aggressive feeds, deep cuts, unconventional strategies) are the operator's prerogative. No "are you sure?" dialogs for valid but aggressive choices.

### 7. Auto-Persistence

Every change is persisted immediately server-side. There is no save button and no "unsaved changes" state. The project is always current. Undo/redo operates on a persistent command history.

## Technology Choices

| Component | Choice | License | Rationale |
|-----------|--------|---------|-----------|
| Backend framework | axum | MIT | Async, WebSocket support, tower middleware, excellent Rust ecosystem |
| Async runtime | tokio | MIT | Industry standard Rust async runtime |
| Serialization | serde + serde_json | MIT | Standard Rust serialization |
| HTTP client (integrations) | reqwest | MIT/Apache | Async HTTP client for Onshape API etc. |
| Geometry kernel | opencascade-rs | LGPL | OpenCascade Rust bindings — B-rep geometry, STEP/STL/DXF import, sectioning, tessellation |
| 2D geometry | geo + geo-clipper | MIT/Apache | Rust computational geometry with Clipper2 bindings |
| Polygon offset | clipper2 (via geo-clipper) | BSL-1.0 | Fast polygon offsetting for toolpath compensation |
| Linear algebra | nalgebra / glam | MIT/Apache | Vectors, matrices, transforms |
| Python bridge | PyO3 + maturin | MIT/Apache | Rust ↔ Python FFI for post-processors |
| Post-processor runtime | Python (embedded via PyO3) | — | Post-processor plugin execution |
| Post-processor plugins | Python entry_points | — | Plugin discovery via importlib.metadata |
| 3D visualization | Three.js | MIT | Industry standard for browser 3D |
| Frontend language | TypeScript | — | Type safety for API contracts |
| Frontend bundler | Vite | MIT | Fast HMR, TypeScript support |

## Why Browser-Based?

- **Cross-platform**: Works on any OS with a browser — no Qt/GTK dependency issues
- **Three.js excellence**: WebGL-based 3D rendering is mature, performant, and well-documented
- **Deployment flexibility**: Can run locally, or be deployed as a shared service
- **Modern UI**: HTML/CSS is far more flexible for UI layout than desktop widget toolkits
- **Lower barrier**: Users don't need to install a desktop application
