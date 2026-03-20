# Implementation Phases

The project starts with 2.5D (DXF/SVG → toolpaths → NC code), then layers 3D support on top. This order makes sense because:

- The backend toolpath pipeline is identical for 2D and 3D inputs (everything works on 2D contours at Z depths)
- The 2D frontend (top-down view with pan/zoom) is much simpler than a full Three.js 3D viewport
- OpenCascade is used lightly at first (wire/face operations) before the heavy STEP reader and tessellation
- Covers the most common hobby CNC workflow: DXF from Inkscape/LibreCAD → profile/pocket → G-code
- NC generation, post-processors, and the full export pipeline are exercised from Phase 2

---

## Phase 1: Scaffolding + 2.5D Geometry Display

**Goal**: A running application where you can upload a DXF or SVG file and view its 2D geometry in a top-down viewport in the browser.

### Backend Tasks (Rust)

1. **Project scaffolding**
   - Create `Cargo.toml` with dependencies (axum, tokio, serde, opencascade-rs, geo)
   - Create `src/main.rs` entry point, `src/lib.rs` module declarations
   - Set up axum app with CORS config, static file serving
   - `GET /api/health` — health check with subsystem status

2. **Core data model (minimal)**
   - `core/project.rs`: `Project` struct with `ProjectType::TwoHalfD`
   - `core/geometry.rs`: `PartGeometry` wrapping `TopoDS_Shape` (wires/faces for 2D), `FaceInfo`, `SurfaceType`, `BoundingBox`
   - `core/units.rs`: `Units` enum

3. **DXF import**
   - `io/dxf_reader.rs`: Parse DXF via OpenCascade, produce `TopoDS_Wire` / `TopoDS_Face`
   - Handle lines, arcs, polylines, circles, splines
   - Closed paths → faces; open paths → wires

4. **SVG import**
   - `io/svg_reader.rs`: Parse SVG paths via OpenCascade, produce wires/faces
   - Same closed/open path distinction as DXF

5. **B-rep persistence**
   - `io/brep_io.rs`: Save/load `.brep` files (OpenCascade native format)
   - `io/project_file.rs`: Save/load `.camproj` JSON with `brep_file` references

6. **API endpoints (minimal)**
   - `POST /api/project` — create 2.5D project
   - `GET /api/project` — get project state
   - `POST /api/project/parts` — upload DXF/SVG file (multipart)
   - `GET /api/project/parts` — list parts
   - `GET /api/project/parts/{id}` — part metadata
   - `GET /api/project/parts/{id}/contour` — get 2D geometry for rendering
   - `GET /api/project/download` — download .camproj
   - `POST /api/project/load` — load .camproj
   - `GET /api/project/history` — undo/redo stack
   - `POST /api/project/undo` / `POST /api/project/redo`

### Frontend Tasks

7. **Frontend scaffolding**
   - `package.json` with TypeScript, Vite (no Three.js yet — 2D only)
   - `vite.config.ts` with API proxy to axum backend
   - `index.html` with layout structure

8. **2D Viewport**
   - Canvas or SVG-based top-down view
   - Render wires as lines/arcs, faces as filled regions
   - Pan and zoom controls
   - Grid with origin axes

9. **Basic UI**
   - Toolbar with "Open File" button (DXF/SVG upload)
   - Status bar showing part info (bounding box, entity count)
   - `api.ts`: REST client for backend communication

### Tests

10. **Backend tests**
    - `test_geometry.rs`: PartGeometry creation, bounding box, face enumeration
    - `test_dxf_reader.rs`: DXF rectangle → expected wire geometry
    - `test_svg_reader.rs`: SVG circle → expected face geometry
    - `test_brep_io.rs`: Save and reload .brep roundtrip
    - `test_api.rs`: File upload, contour retrieval (using axum-test)
    - Test fixtures: `rectangle.dxf`, `circle.svg`, `profile_with_arcs.dxf`

### Deliverable

User runs `cargo run`, opens `localhost:8000` in browser, uploads a DXF, sees the 2D geometry rendered top-down with pan/zoom and a grid/axes reference. Project save/load works.

---

## Phase 2: 2.5D Toolpath Operations

**Goal**: Define tools, setups, stock, and machining operations. Generate and visualize toolpaths on 2D geometry.

### Backend Tasks (Rust)

1. **Tool definitions**
   - `core/tool.rs`: `Tool` (with `tool_number`, `machine`), `ToolType`, `ToolLibrary`
   - Tool CRUD API: project tools + global library + import/export

2. **Setup framework**
   - `core/setup.rs`: `Setup` struct (WCS, stock, clearance height)
   - Setup CRUD API endpoints
   - WCS inheritance to child operations

3. **Stock setup**
   - Stock definition on setup (box or cylinder)
   - Auto-suggest stock from part bounding box

4. **Operation framework**
   - `core/operation.rs`: `Operation` struct with `setup_id`, type-specific params, `pocket_entry`
   - Operation CRUD API: create, update, delete, reorder, duplicate, get by ID
   - Setup/WCS/stock/clearance inheritance with per-operation override

5. **Polygon offset**
   - `toolpath/offset.rs`: `offset_polygon()`, `iterative_offset()` using geo-clipper
   - Convert OpenCascade wires/edges → `geo::MultiPolygon` for offset operations

6. **Depth strategy**
   - `toolpath/depth_strategy.rs`: `compute_depth_passes()`

7. **Facing generator**
   - `toolpath/facing.rs`: Zigzag raster pattern across stock top

8. **Profile generator**
   - `toolpath/profile.rs`: Contour following with inside/outside/on-line
   - Lead-in arc support
   - CAM mode (polygon offset) and Controller mode (exact geometry, arcs preserved)
   - Tab support for sheet cutting

9. **Pocket generator**
   - `toolpath/pocket.rs`: Contour-parallel and zigzag strategies
   - Entry type: plunge, helix, ramp

10. **Toolpath API**
    - `POST /api/project/operations/{id}/generate` — trigger generation (async)
    - `POST /api/project/operations/generate-all`
    - `GET /api/project/operations/{id}/toolpath` — get toolpath data (with `start_position`)
    - WebSocket: progress, completion, error, cancel

### Frontend Tasks

11. **Operations panel**: List, add, delete, reorder, duplicate operations (grouped by setup)
12. **Properties panel**: Edit operation parameters, setup inheritance indicators
13. **Setup panel**: Setup CRUD, WCS/stock/clearance height editing
14. **Tools panel**: Tool library CRUD with tool number and machine fields
15. **Stock setup dialog**: Define stock dimensions per setup
16. **Toolpath visualization**: Render toolpath lines in 2D viewport (rapid=red dashed, feed=blue, plunge=green)

### Tests

17. **Toolpath tests**
    - `test_offset.rs`: Square offset inward/outward → expected dimensions
    - `test_facing.rs`: Facing on stock → expected zigzag pattern
    - `test_profile.rs`: Square profile → expected offset contour, arc preservation in controller mode
    - `test_pocket.rs`: Square pocket → expected offset loops, helix entry
    - `test_depth_strategy.rs`: Depth pass computation

### Deliverable

User can create setups, define stock, add tools, create facing/profile/pocket operations on DXF/SVG geometry, generate toolpaths, and see them visualized over the 2D geometry.

---

## Phase 3: NC Code Generation + Export

**Goal**: Export machine-ready NC code through pluggable post-processors. Complete end-to-end workflow for 2.5D projects.

### Backend Tasks

1. **NC intermediate representation** (Rust)
   - `nc/ir.rs`: `NCBlock`, `BlockType` enum
   - `nc/compiler.rs`: `compile_program()` — operations → NCBlock list
   - Setup-aware compilation: clearance retractions between setups

2. **PyO3 bridge** (Rust)
   - `nc/bridge.rs`: NCBlock Rust → Python conversion, post-processor discovery and invocation

3. **Post-processor framework** (Python)
   - `postprocessors/src/camproject_post/base.py`: `PostProcessor` ABC, `ProgramContext`, Python `NCBlock`
   - Entry-point based plugin discovery
   - `tool_call_by_name` option for Heidenhain

4. **Built-in post-processors** (Python)
   - `linuxcnc.py`, `grbl.py`, `marlin.py`, `generic_fanuc.py`

5. **Export API** (Rust)
   - `GET /api/postprocessors` — list available (calls PyO3 bridge)
   - `POST /api/project/export/preview` — preview NC code (with `setup_id` filter)
   - `POST /api/project/export` — download NC file
   - `POST /api/project/operations/{id}/export/preview` — single-operation shortcut

6. **Undo/redo**
   - `CommandHistory` with JSON patches
   - `POST /api/project/undo`, `POST /api/project/redo`

### Frontend Tasks

7. **NC preview panel**: Syntax-highlighted NC code display with post-processor dropdown
8. **Export dialog**: Post-processor selection, setup/operation filter, download
9. **Undo/redo buttons**: Tooltips from history API, keyboard shortcuts (Ctrl+Z / Ctrl+Shift+Z)

### Tests

10. **NC tests**
    - `test_nc_compiler.rs`: Simple toolpath → expected NCBlocks, setup retraction handling
    - `test_postprocessors.py`: NCBlocks → expected G-code for each post-processor
    - Round-trip: known DXF geometry → toolpath → NC code → verify against reference output

### Deliverable

Complete 2.5D workflow: import DXF/SVG → define operations → generate toolpaths → preview and export NC code for LinuxCNC, Grbl, Marlin, or Fanuc controllers. This is the first **usable release** for hobby CNC users.

---

## Phase 4: Drill Cycles + Advanced Post-Processors

**Goal**: Drilling operations, canned cycle support, and Sinumerik/Heidenhain post-processors.

### Tasks

1. `toolpath/drill.rs`: Drill cycle generation (simple, peck, spot, bore, tap)
2. Dual output: explicit moves (universal) + `CycleDefine`/`CycleCall` blocks (for controllers that support them)
3. `sinumerik.py` post-processor: Sinumerik-specific formatting, `/1`-`/8` block delete, conditional jumps
4. `heidenhain.py` post-processor: Full conversational format override, `TOOL CALL` by name, `BLK FORM`, `CYCL DEF`
5. Optional operations: block delete (`/` prefix) and conditional jump strategies
6. Cutter compensation in NC output: `G41`/`G42` for G-code, `RL`/`RR` for Heidenhain

### Tests

7. `test_drill.rs`: Drill point generation, peck cycle parameters
8. `test_sinumerik.py`, `test_heidenhain.py`: NC output verification against reference

### Deliverable

User can create drill operations, use canned cycles on supporting controllers, and export to Sinumerik and Heidenhain TNC.

---

## Phase 5: 3D Projects (STEP/STL + 3D Viewport)

**Goal**: Add 3D project support — STEP/STL import, B-rep solids, Three.js 3D viewport, face selection.

### Backend Tasks

1. **STEP import**
   - `io/step_reader.rs`: STEP → `TopoDS_Shape` via OpenCascade `STEPControl_Reader`
   - Extract `FaceInfo` metadata from the B-rep

2. **STL import** (degraded)
   - `io/stl_reader.rs`: STL → `TopoDS_Shape` via OpenCascade sewing (`BRepBuilderAPI_Sewing`)
   - Triangular face shell — functional but without logical face reconstruction

3. **B-rep tessellation**
   - `PartGeometry::tessellate()`: B-rep → `TessellatedMesh` with `face_ids` per triangle
   - Configurable deflection parameter

4. **Mesh API**
   - `GET /api/project/parts/{id}/mesh?deflection=0.1` — tessellated mesh with face IDs
   - `GET /api/project/parts/{id}/contour?z={z}` — exact section at Z height

5. **Face selection**
   - `POST /api/project/parts/{id}/orient` — orient part by face normal (accepts `face_id`)
   - Backend reads face normal from B-rep, computes orientation transform

6. **B-rep sectioning for toolpaths**
   - `toolpath/slicer.rs`: `BRepAlgoAPI_Section` (shape vs. plane) → exact curves
   - Convert to `geo::MultiPolygon` for offset operations
   - Preserve exact arcs for profile operations in controller compensation mode

7. **3D project type**
   - `ProjectType::ThreeD` — accepts STEP/STL, uses B-rep geometry
   - `POST /api/project` with `project_type: "3d"`

### Frontend Tasks

8. **Three.js 3D viewport**
   - `viewport/scene.ts`: Scene, renderer, lights
   - `viewport/camera.ts`: OrbitControls (orbit, pan, zoom)
   - `viewport/mesh-loader.ts`: Fetch tessellated mesh, create BufferGeometry with face_ids attribute
   - `viewport/grid.ts`: XY grid + axes

9. **Face interaction**
   - Raycasting on tessellated mesh → triangle index → `face_ids` lookup → B-rep face ID
   - Face highlighting on hover
   - Click face for orientation ("set this face as top")
   - Right-click context menu (orient, set WCS, add operation on face)

10. **Toolpath visualization in 3D**
    - `viewport/toolpath-renderer.ts`: Render toolpath lines over the 3D mesh
    - `viewport/stock-renderer.ts`: Wireframe stock box

### Tests

11. `test_step_reader.rs`: STEP import → expected face count, bounding box
12. `test_stl_reader.rs`: STL sewing → valid TopoDS_Shape
13. `test_tessellation.rs`: Tessellate → expected triangle count, face_ids consistency
14. `test_slicer.rs`: Section B-rep at Z → expected exact curves

### Deliverable

User can create 3D projects, import STEP files, view the model in a 3D viewport, click faces to orient the part, and run all existing toolpath operations on sliced geometry. STL is supported as a degraded fallback.

---

## Phase 6: Advanced Features

**Goal**: Polish, performance, and advanced capabilities.

### Tasks

1. **Part update pipeline** (from `09-part-update.md`): Geometry diff, registration, operation audit, user review
2. **Stock simulation**: Z-buffer material removal preview in viewport (API contracts defined in `02-api-design.md`)
3. **3D toolpath strategies**: Waterline / Z-level roughing, raster finishing (future)
4. **CAD integrations**: Onshape API, FreeCAD CLI bridge, watch folder (from `08-integrations.md`)
5. **Performance**: Optimize toolpath generation for complex geometries, background computation via tokio tasks

### Deliverable

Production-quality tool with part update handling, simulation preview, and CAD system integrations.

---

## Priority and Dependencies

```
Phase 1 (2.5D scaffolding) ─→ Phase 2 (toolpaths) ─→ Phase 3 (NC export)
                                                            │
                                              Phase 4 (drill + advanced post) ←─┘
                                                            │
                                              Phase 5 (3D projects) ←───────────┘
                                                            │
                                              Phase 6 (advanced) ←──────────────┘
```

Phases 1-3 form the **minimum viable product**: import DXF → define operations → export G-code. This is a complete, usable tool for hobby CNC users.

Phase 4 adds drill operations and industrial post-processors (Sinumerik, Heidenhain).

Phase 5 layers 3D support on top of the existing pipeline — the toolpath generators don't change, only the geometry input and frontend visualization.

Phase 6 is ongoing polish and advanced features.
