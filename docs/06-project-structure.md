# Project Structure & Build Configuration

## Directory Layout

```
CAMproject/
├── LICENSE                         # MIT License
├── CLAUDE.md                       # Claude Code guidance
├── pyproject.toml                  # Python package config (PEP 621 + hatchling)
├── docs/                           # Design documentation (these files)
│   ├── 00-overview.md
│   ├── 01-data-model.md
│   ├── 02-api-design.md
│   ├── 03-nc-and-postprocessors.md
│   ├── 04-toolpath-algorithms.md
│   ├── 05-frontend-design.md
│   ├── 06-project-structure.md
│   └── 07-implementation-phases.md
├── src/
│   └── camproject/
│       ├── __init__.py             # Version string
│       ├── __main__.py             # `python -m camproject` entry point
│       ├── server.py               # FastAPI app, CORS, static files, lifespan
│       ├── api/
│       │   ├── __init__.py
│       │   ├── routes.py           # REST API endpoints
│       │   └── websocket.py        # WebSocket handler for progress
│       ├── core/
│       │   ├── __init__.py
│       │   ├── project.py          # Project container, save/load
│       │   ├── geometry.py         # PartGeometry, StockDefinition
│       │   ├── tool.py             # Tool definitions
│       │   ├── operation.py        # Operation types and params
│       │   ├── toolpath.py         # Toolpath, ToolpathSegment
│       │   └── units.py            # mm/inch enum and conversion
│       ├── toolpath/
│       │   ├── __init__.py
│       │   ├── slicer.py           # Mesh → 2D cross-sections
│       │   ├── offset.py           # Polygon offset (pyclipr wrapper)
│       │   ├── facing.py           # Facing toolpath generator
│       │   ├── profile.py          # Profile toolpath generator
│       │   ├── pocket.py           # Pocket toolpath generator
│       │   ├── drill.py            # Drill cycle generator (Phase 4)
│       │   ├── ordering.py         # Segment ordering optimization
│       │   └── depth_strategy.py   # Multi-pass Z stepping
│       ├── nc/
│       │   ├── __init__.py
│       │   ├── ir.py               # NCBlock, BlockType
│       │   ├── compiler.py         # Toolpath → NCBlock list
│       │   ├── base.py             # PostProcessor ABC
│       │   └── registry.py         # Plugin discovery via entry_points
│       ├── postprocessors/
│       │   ├── __init__.py
│       │   ├── linuxcnc.py         # LinuxCNC post-processor
│       │   ├── grbl.py             # Grbl post-processor
│       │   ├── marlin.py           # Marlin post-processor
│       │   └── generic_fanuc.py    # Generic Fanuc post-processor
│       ├── io/
│       │   ├── __init__.py
│       │   ├── stl_reader.py       # STL → PartGeometry (via trimesh)
│       │   ├── dxf_reader.py       # DXF → PartGeometry (via ezdxf)
│       │   ├── svg_reader.py       # SVG → PartGeometry (via svgpathtools)
│       │   ├── step_reader.py      # STEP → PartGeometry (stub → Phase 5)
│       │   └── project_file.py     # .camproj save/load
│       └── utils/
│           ├── __init__.py
│           └── math_utils.py       # Arc fitting, geometric helpers
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
    ├── conftest.py
    ├── test_geometry.py
    ├── test_slicer.py
    ├── test_offset.py
    ├── test_facing.py
    ├── test_profile.py
    ├── test_pocket.py
    ├── test_nc_compiler.py
    ├── test_postprocessors.py
    ├── test_dxf_reader.py
    ├── test_svg_reader.py
    ├── test_api.py
    └── fixtures/
        ├── cube.stl
        ├── simple_pocket.stl
        ├── rectangle.dxf
        └── circle.svg
```

## pyproject.toml

```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "camproject"
version = "0.1.0"
description = "Browser-based CAM tool for CNC milling NC code generation"
readme = "README.md"
license = "MIT"
requires-python = ">=3.11"
authors = [
    { name = "Thomas Van Riel" },
]
dependencies = [
    "fastapi>=0.110",
    "uvicorn[standard]>=0.27",
    "python-multipart>=0.0.7",    # For file uploads
    "trimesh>=4.0",
    "numpy>=1.24",
    "shapely>=2.0",
    "pyclipr>=0.2",
    "ezdxf>=1.3",
    "svgpathtools>=1.6",
]

[project.optional-dependencies]
step = ["OCP>=7.7"]
dev = [
    "pytest>=8.0",
    "pytest-asyncio>=0.23",
    "httpx>=0.27",                # For testing FastAPI with TestClient
    "ruff>=0.3",
    "mypy>=1.8",
]

[project.scripts]
camproject = "camproject.__main__:main"

[project.entry-points."camproject.postprocessors"]
linuxcnc = "camproject.postprocessors.linuxcnc:LinuxCNCPost"
grbl = "camproject.postprocessors.grbl:GrblPost"
marlin = "camproject.postprocessors.marlin:MarlinPost"
fanuc = "camproject.postprocessors.generic_fanuc:GenericFanucPost"

[tool.ruff]
target-version = "py311"
line-length = 100
src = ["src"]

[tool.ruff.lint]
select = ["E", "F", "I", "UP", "B"]

[tool.pytest.ini_options]
testpaths = ["tests"]
asyncio_mode = "auto"

[tool.mypy]
python_version = "3.11"
strict = true
```

## Development Commands

```bash
# Install in development mode
pip install -e ".[dev]"

# Run the server (backend)
python -m camproject                    # Production: serves frontend from frontend/dist/
python -m camproject --dev --port 8000  # Development: API only, CORS enabled

# Frontend development
cd frontend
npm install
npm run dev                             # Vite dev server on :5173, proxies /api to :8000

# Frontend build (for production)
cd frontend
npm run build                           # Outputs to frontend/dist/

# Run tests
pytest                                  # All tests
pytest tests/test_pocket.py             # Single test file
pytest tests/test_pocket.py::test_square_pocket  # Single test
pytest -x                               # Stop on first failure
pytest -k "profile"                     # Tests matching keyword

# Lint
ruff check src/
ruff format src/

# Type check
mypy src/camproject/
```

## Module Dependency Rules

```
api/  →  core/, toolpath/, nc/, postprocessors/, io/
          (API layer can import everything)

core/ →  (no internal dependencies, only external: trimesh, shapely, numpy)

toolpath/ →  core/
              (toolpath generators use core types)

nc/   →  core/
          (NC compiler uses core types)

postprocessors/ →  nc/
                    (post-processors extend nc/base.py)

io/   →  core/
          (readers produce core types)

utils/ →  (no internal dependencies)
```

No circular dependencies. The `core/` package is the foundation that everything depends on but depends on nothing internal.
