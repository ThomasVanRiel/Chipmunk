# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

CAMproject is a browser-based CAM (Computer-Aided Manufacturing) tool for generating NC code for CNC milling machines. It accepts 3D models (STL, STEP) and 2D drawings (DXF, SVG) as input, lets the user define machining operations, generates toolpaths, and exports controller-agnostic NC code through pluggable post-processors.

- **License**: MIT
- **Remote**: `git@github.com:ThomasVanRiel/CAMproject.git`

## Architecture

**Backend**: Python (FastAPI) — handles geometry processing, toolpath generation, NC compilation.
**Frontend**: TypeScript + Three.js — browser-based 3D viewport and UI panels.
Communication via REST API + WebSocket (for toolpath generation progress).

### Data Flow

```
File/CAD Import → PartGeometry → Operation (geometry + tool + params)
    → Toolpath → NCBlock IR → PostProcessor → NC code string
```

### Module Dependency Rules (no circular deps)

```
api/            → core/, toolpath/, nc/, postprocessors/, io/, integrations/
core/           → (no internal deps — only trimesh, shapely, numpy)
toolpath/       → core/
nc/             → core/
postprocessors/ → nc/
io/             → core/
integrations/   → io/, core/
utils/          → (no internal deps)
```

Key principle: `core/`, `toolpath/`, `nc/`, `io/` have **zero web framework dependencies**. They are pure computational Python, independently testable.

### Three-Layer NC Generation

1. **Toolpath** — pure geometry segments (rapid/linear/arc with XYZ coordinates)
2. **NCBlock IR** — controller-neutral program (adds spindle, tools, coolant, compensation, cycles, optional skips)
3. **Post-processor** — formats to machine-specific output (G-code, Heidenhain conversational, Sinumerik)

Post-processors are pluggable via Python entry_points. Built-in: LinuxCNC, Grbl, Marlin, Generic Fanuc, Sinumerik, Heidenhain TNC.

### Cutter Compensation

Operations support two modes: **CAM mode** (software computes offset toolpath) and **Controller mode** (emits G41/G42 or RL/RR, controller applies offset at runtime). Roughing typically uses CAM mode; finishing uses controller mode so the operator can fine-tune dimensions.

### Optional Operations

Operations can be marked optional with a skip level (1-9). Post-processors implement this via block delete (`/` prefix) or conditional jumps (controller-specific labels/variables), depending on the target machine.

### Canned Cycles

The IR supports `CYCLE_DEFINE`/`CYCLE_CALL` blocks. Toolpath generators always produce explicit moves as fallback. Post-processors that declare cycle support emit native cycles (G81/G83 for G-code, CYCL DEF for Heidenhain); others use the expanded moves.

## Build & Development Commands

```bash
# Install dependencies (uv manages the virtualenv automatically)
uv sync                                  # Install all deps including dev
uv sync --extra step                     # Also install STEP/OpenCascade support

# Run the server
uv run python -m camproject                     # Production: serves frontend from frontend/dist/
uv run python -m camproject --dev --port 8000   # Development: API only, CORS enabled

# Frontend
cd frontend && npm install
cd frontend && npm run dev               # Vite dev server on :5173, proxies /api to :8000
cd frontend && npm run build             # Build to frontend/dist/

# Tests
uv run pytest                                   # All tests
uv run pytest tests/test_pocket.py              # Single file
uv run pytest tests/test_pocket.py::test_name   # Single test
uv run pytest -x                                # Stop on first failure
uv run pytest -k "profile"                      # Match keyword

# Lint & type check
uv run ruff check src/
uv run ruff format src/
uv run mypy src/camproject/

# Add a dependency
uv add <package>                         # Runtime dependency
uv add --dev <package>                   # Dev dependency
```

## Design Documents

Detailed design docs live in `docs/`:

| Doc | Contents |
|-----|----------|
| `00-overview.md` | Architecture, tech choices, design principles |
| `01-data-model.md` | Core types: Project, PartGeometry, Tool, Operation, Toolpath |
| `02-api-design.md` | REST API + WebSocket spec with request/response examples |
| `03-nc-and-postprocessors.md` | NCBlock IR, PostProcessor ABC, canned cycles, optional operations, Heidenhain/Sinumerik details |
| `04-toolpath-algorithms.md` | Slicing, offset, facing, profile, pocket, drill algorithms |
| `05-frontend-design.md` | Three.js viewport, UI layout, panels, Vite build |
| `06-project-structure.md` | Directory tree, pyproject.toml, dependency rules |
| `07-implementation-phases.md` | Phased task breakdown with deliverables |
| `08-integrations.md` | CAD integrations: Onshape API, FreeCAD, watch folder |
| `09-part-update.md` | Geometry change handling: diff, alignment, operation audit |

When implementing a feature, read the relevant design doc first. The docs are the source of truth for architectural decisions.

## Key Design Decisions

- **Heidenhain is not a G-code dialect** — it's a completely different language (conversational format). The PostProcessor `generate()` method is designed to be fully overridable for this reason.
- **Part provenance** tracks where geometry came from (file path, Onshape doc ID, etc.) so it can be refreshed from source.
- **Part update pipeline** (diff → registration → change report → operation audit → user review) ensures CAM operations are preserved when the CAD model changes. Never silently break a project.
- **Post-processor plugin system** uses Python entry_points so third-party packages can register custom post-processors.
