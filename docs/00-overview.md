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
│                  FastAPI Backend (Python)               │
│                                                         │
│  ┌─────────┐  ┌──────────┐  ┌────┐  ┌───────────────┐   │
│  │  api/   │→ │  core/   │→ │ nc/│→ │postprocessors/│   │
│  │ routes  │  │ geometry │  │ IR │  │  linuxcnc     │   │
│  │ websock │  │ tools    │  │comp│  │  grbl         │   │
│  └─────────┘  │ ops      │  └────┘  │  fanuc        │   │
│               └──────────┘          └───────────────┘   │
│                    ↑                                    │
│  ┌─────────┐  ┌──────────┐                              │
│  │   io/   │  │toolpath/ │                              │
│  │ stl     │  │ slicer   │                              │
│  │ dxf     │  │ offset   │                              │
│  │ svg     │  │ facing   │                              │
│  │ step    │  │ profile  │                              │
│  └─────────┘  │ pocket   │                              │
│               └──────────┘                              │
└─────────────────────────────────────────────────────────┘
```

## Design Principles

### 1. Separation of Concerns

`core/`, `toolpath/`, `nc/`, and `io/` have **zero web framework dependencies**. They are pure computational Python modules, independently testable without a running server or browser. The `api/` layer is a thin adapter between HTTP and the core logic.

### 2. Data Flow is Unidirectional

```
File Import → PartGeometry → Operation (geometry + tool + params)
    → Toolpath → NCBlock list → PostProcessor → NC code string
```

Each stage is independently testable and produces a well-defined output.

### 3. Controller Agnosticism

Toolpaths are generated as abstract segment sequences (rapid, linear, arc). These are compiled to a controller-neutral intermediate representation (`NCBlock` list). Only the final post-processor step formats machine-specific G-code.

### 4. Plugin Extensibility

New operations (subclass `Operation`), new post-processors (subclass `PostProcessor`, register via entry point), new importers (add reader in `io/`), and new toolpath strategies (add module in `toolpath/`) can be added without modifying existing code.

### 5. No Singleton State

The `Project` object is the root of all state. The server could theoretically serve multiple projects. All computation functions are pure (inputs → outputs).

## Technology Choices

| Component | Choice | License | Rationale |
|-----------|--------|---------|-----------|
| Backend framework | FastAPI | MIT | Async, WebSocket support, modern Python |
| 3D visualization | Three.js | MIT | Industry standard for browser 3D, excellent OrbitControls |
| Frontend language | TypeScript | — | Type safety for API contracts, Three.js has good TS types |
| STL/mesh handling | trimesh | MIT | Mature mesh library with slicing, bounding boxes, transforms |
| 2D geometry | Shapely | BSD | GEOS-backed boolean ops, buffering, offsetting |
| Polygon offset | pyclipr | MIT | Fast Clipper2 bindings for toolpath offset calculations |
| DXF import | ezdxf | MIT | Full DXF version support, actively maintained |
| SVG import | svgpathtools | MIT | SVG path parsing to Bezier segments |
| STEP import | OCP/build123d | LGPL | OpenCascade wrapper, deferred (heavy ~300MB dep) |
| Python linting | ruff | MIT | Fast, replaces flake8+isort+black |
| Testing | pytest | MIT | Standard Python testing |

## Why Browser-Based?

- **Cross-platform**: Works on any OS with a browser — no Qt/GTK dependency issues
- **Three.js excellence**: WebGL-based 3D rendering is mature, performant, and well-documented
- **Deployment flexibility**: Can run locally, or be deployed as a shared service
- **Modern UI**: HTML/CSS is far more flexible for UI layout than desktop widget toolkits
- **Lower barrier**: Users don't need to install a desktop application
