# Implementation Phases

## Phase 1: Scaffolding + GUI Shell + STL Display

**Goal**: A running application where you can upload an STL file and view it in a 3D viewport in the browser.

### Backend Tasks (Rust)

1. **Project scaffolding**
   - Create `Cargo.toml` with dependencies (axum, tokio, serde, stl_io)
   - Create `src/main.rs` entry point, `src/lib.rs` module declarations
   - Set up axum app with CORS config, static file serving

2. **Core data model (minimal)**
   - `core/geometry.rs`: `PartGeometry` struct wrapping a `TriMesh`, `StockDefinition`, `BoundingBox`
   - `core/project.rs`: `Project` struct holding parts list
   - `core/units.rs`: `Units` enum

3. **STL import**
   - `io/stl_reader.rs`: Load STL via `stl_io`, return `PartGeometry`

4. **API endpoints (minimal)**
   - `POST /api/project` — create project
   - `GET /api/project` — get project state
   - `POST /api/project/parts` — upload STL file (multipart)
   - `GET /api/project/parts/{id}/mesh` — get mesh data for Three.js

### Frontend Tasks

5. **Frontend scaffolding**
   - `package.json` with Three.js, TypeScript, Vite
   - `vite.config.ts` with API proxy
   - `index.html` with layout structure

6. **3D Viewport**
   - `viewport/scene.ts`: Three.js scene, renderer, lights
   - `viewport/camera.ts`: OrbitControls setup
   - `viewport/grid.ts`: Grid helper + axes
   - `viewport/mesh-loader.ts`: Fetch mesh from API, create BufferGeometry

7. **Basic UI**
   - Toolbar with "Open File" button (triggers file upload)
   - Status bar showing part info
   - `api.ts`: REST client for backend communication

### Tests

8. **Backend tests**
   - `test_geometry.rs`: PartGeometry creation, bounding box
   - `test_api.rs`: File upload, mesh retrieval (using axum-test)
   - Test fixture: `cube.stl` (simple 10x10x10 cube)

### Deliverable

User runs `cargo run`, opens `localhost:8000` in browser, uploads an STL, sees the mesh rendered with orbit/pan/zoom and a grid/axes reference.

---

## Phase 2: 2.5D Toolpath Operations

**Goal**: Define tools, stock, and machining operations. Generate and visualize toolpaths.

### Backend Tasks (Rust)

1. **Tool definitions**
   - `core/tool.rs`: `Tool`, `ToolType` structs/enums
   - Tool CRUD API endpoints

2. **Stock setup**
   - Stock API endpoints
   - Auto-detect stock from part bounding box (with user-adjustable margin)

3. **Operation framework**
   - `core/operation.rs`: `Operation` struct with type-specific params
   - `core/toolpath.rs`: `Toolpath`, `ToolpathSegment`, `MoveType`
   - Operation CRUD API endpoints

4. **Mesh slicing**
   - `toolpath/slicer.rs`: Custom mesh-plane intersection → geo polygons

5. **Polygon offset**
   - `toolpath/offset.rs`: `offset_polygon()`, `iterative_offset()` using geo-clipper

6. **Depth strategy**
   - `toolpath/depth_strategy.rs`: `compute_depth_passes()`

7. **Facing generator**
   - `toolpath/facing.rs`: Zigzag raster pattern across stock top

8. **Profile generator**
   - `toolpath/profile.rs`: Contour following with tool compensation, inside/outside

9. **Pocket generator**
   - `toolpath/pocket.rs`: Contour-parallel strategy with iterative inward offset

10. **Toolpath API**
    - `POST /api/project/operations/{id}/generate` — trigger generation
    - `GET /api/project/operations/{id}/toolpath` — get toolpath data
    - WebSocket for generation progress

### Frontend Tasks

11. **Operations panel**: List, add, delete, reorder operations
12. **Properties panel**: Edit operation parameters
13. **Tools panel**: Tool library CRUD
14. **Stock setup dialog**: Define stock dimensions
15. **Toolpath visualization**: Render toolpath lines in viewport (rapid=red, feed=blue, plunge=green)

### Tests

16. **Toolpath tests**
    - `test_slicer.rs`: Cube slice at known Z → expected rectangle
    - `test_offset.rs`: Square offset inward/outward → expected dimensions
    - `test_facing.rs`: Facing on stock → expected zigzag pattern
    - `test_profile.rs`: Square profile → expected offset contour
    - `test_pocket.rs`: Square pocket → expected offset loops

### Deliverable

User can define stock, add a tool, create facing/profile/pocket operations, generate toolpaths, and see them visualized over the part mesh.

---

## Phase 3: NC Code Generation

**Goal**: Export machine-ready NC code through pluggable post-processors.

### Backend Tasks

1. **NC intermediate representation** (Rust)
   - `nc/ir.rs`: `NCBlock`, `BlockType` enum
   - `nc/compiler.rs`: `compile_program()` — operations → NCBlock list

2. **PyO3 bridge** (Rust)
   - `nc/bridge.rs`: NCBlock Rust → Python conversion, post-processor discovery and invocation

3. **Post-processor framework** (Python)
   - `postprocessors/src/camproject_post/base.py`: `PostProcessor` ABC, `ProgramContext`, Python `NCBlock`
   - Entry-point based plugin discovery

4. **Built-in post-processors** (Python)
   - `linuxcnc.py`, `grbl.py`, `marlin.py`, `generic_fanuc.py`

5. **Export API** (Rust)
   - `GET /api/postprocessors` — list available (calls PyO3 bridge)
   - `POST /api/project/export/preview` — preview NC code
   - `POST /api/project/export` — download NC file

6. **Project persistence** (Rust)
   - `io/project_file.rs`: Save/load `.camproj` JSON (via serde)
   - `POST /api/project/load`

### Frontend Tasks

7. **NC preview panel**: Syntax-highlighted NC code display
8. **Export dialog**: Post-processor selection, operation filter, download
9. **Project load**: Open project files

### Tests

10. **NC tests**
    - `test_nc_compiler.rs`: Simple toolpath → expected NCBlocks
    - `test_postprocessors.py`: NCBlocks → expected G-code for each post-processor (Python tests)
    - Round-trip: known geometry → toolpath → NC code → verify against reference

### Deliverable

User can generate NC code, preview it in the browser, select a post-processor, and download the file ready for their CNC machine.

---

## Phase 4: DXF/SVG Import, Drill Cycles, Advanced Post-Processors

**Goal**: Accept 2D drawing inputs, support drilling operations, and add Sinumerik/Heidenhain post-processors.

### Tasks

1. `io/dxf_reader.rs`: DXF → geo geometry (lines, arcs, polylines, circles) via dxf-rs
2. `io/svg_reader.rs`: SVG → geo geometry (paths, circles, rectangles) via usvg
3. Update `core/geometry.rs`: Support 2D contour-based PartGeometry with extrusion depth
4. `toolpath/drill.rs`: Drill cycle generation (simple, peck, spot)
5. Helical entry option for pocket and profile operations
6. Ramp entry option for profile operations
7. Tab support for profile operations (hold-down tabs for sheet cutting)
8. `sinumerik.py` and `heidenhain.py` post-processors
9. `test_dxf_reader.rs`, `test_svg_reader.rs` with fixture files

### Deliverable

User can import DXF/SVG files, create 2.5D operations on 2D geometry, use drill cycles, and export to Sinumerik and Heidenhain controllers.

---

## Phase 5: STEP Support, 3D Operations

**Goal**: Handle solid models and 3D toolpath strategies.

### Tasks

1. `io/step_reader.rs`: STEP → PartGeometry via opencascade-rs (behind `step` feature flag)
2. 3D roughing: Waterline / Z-level strategy with rest machining
3. 3D finishing: Raster, spiral, pencil trace strategies
4. Stock simulation: Z-buffer material removal preview in viewport
5. Background computation: Move heavy toolpath generation to tokio tasks with progress
6. Performance: Optimize toolpath generation for complex geometries

### Deliverable

User can import STEP files, use 3D toolpath strategies, and preview stock removal.

---

## Priority and Dependencies

```
Phase 1 ─→ Phase 2 ─→ Phase 3
                         │
              Phase 4 ←──┘ (can start after Phase 2 core is done)
                         │
              Phase 5 ←──┘ (can start after Phase 3)
```

Phase 4 is partially independent — DXF/SVG import and drill cycles can be developed in parallel with Phase 3 NC code generation, as long as Phase 2's operation framework is in place.
