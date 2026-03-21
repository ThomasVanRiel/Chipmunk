# B-Rep as Primary Geometry Representation

**Decision**: B-rep is the internal geometry representation. OpenCascade is a hard dependency. Triangle meshes are generated on demand for the frontend viewport only.

This document maps out what changes and where decisions are still needed.

---

## What B-Rep gives us

With OpenCascade as the geometry kernel, `PartGeometry` wraps a `TopoDS_Shape` — the root of OpenCascade's topology:

```
TopoDS_Shape (compound/solid)
├── Shell
│   ├── Face (planar, cylindrical, conical, spherical, toroidal, NURBS, ...)
│   │   ├── Wire (closed loop of edges bounding the face)
│   │   │   ├── Edge (line, arc, circle, ellipse, B-spline, ...)
│   │   │   │   ├── Vertex (start)
│   │   │   │   └── Vertex (end)
│   │   │   └── ...more edges
│   │   └── ...inner wires (holes in the face)
│   └── ...more faces
└── ...more shells
```

Every face knows its surface type, normal direction, and parametric bounds. Every edge knows its curve type and the two faces it borders. This is what makes face selection, hole detection, and exact slicing possible.

---

## New PartGeometry data model

```rust
pub struct PartGeometry {
    pub id: Uuid,
    pub name: String,
    pub source_format: String,            // "stl", "dxf", "svg", "step"

    // Primary representation — OpenCascade B-rep shape
    #[serde(skip)]
    pub shape: TopoDS_Shape,              // The B-rep solid/compound

    // Cached face metadata for the API/frontend (serializable)
    pub faces: Vec<FaceInfo>,

    // Transform applied to the imported geometry
    pub transform: [[f64; 4]; 4],

    // Provenance + update history (unchanged)
    pub provenance: Option<PartProvenance>,
    pub update_history: Vec<PartUpdate>,
}

pub struct FaceInfo {
    pub id: u32,                          // Stable face index within the shape
    pub surface_type: SurfaceType,        // Plane, Cylinder, Cone, Sphere, Torus, BSpline, ...
    pub normal: Option<[f64; 3]>,         // Outward normal (only meaningful for planar faces)
    pub area: f64,
    pub bounding_box: BoundingBox,
}

pub enum SurfaceType {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    BSpline,
    Other,
}
```

**Decision needed — FaceInfo granularity**:
> _How much face metadata should be cached in FaceInfo? Minimal (id, type, normal, area) or richer (adjacent face IDs, edge list, parametric bounds)?_

---

## Import pipeline — everything goes through OpenCascade

### STEP (native B-rep)

STEP is the ideal input. OpenCascade reads it directly into `TopoDS_Shape` — no conversion loss.

```rust
// io/step_reader.rs
pub fn read_step(path: &Path) -> Result<TopoDS_Shape> {
    // opencascade-rs STEPControl_Reader
}
```

### STL (tessellation → B-rep reconstruction)

STL gives triangle soup. OpenCascade can sew triangles into a `TopoDS_Shape` via `BRepBuilderAPI_Sewing`, but the result is a shell of planar triangular faces — you don't get the original cylinders, fillets, etc. back.

Options for STL:
- **A**: Sew into B-rep shell. You get topology (face adjacency, edges) but every face is a tiny triangle. Face grouping becomes essential to make this usable.
- **B**: Store as a `TopoDS_Shape` from sewing, but also run face clustering (merge coplanar/co-cylindrical adjacent triangles into logical faces). This is geometry reconstruction — complex but very valuable.
- **C**: Accept degraded experience for STL. Import it, sew it, let the user work with it. STEP is the recommended format.

**Decision needed — STL strategy**:
> _Option A (raw sew), B (reconstruct logical faces), or C (degraded but functional)? B is the most work but the most useful. C is pragmatic given the target audience likely has STEP access._

### DXF / SVG (2.5D projects)

DXF and SVG are 2D inputs — they belong to **2.5D projects** (see decision #3). They are not mixed with 3D STEP/STL parts in the same project.

DXF gives 2D entities (lines, arcs, polylines, circles, splines). SVG gives paths, circles, rectangles. Both are converted to OpenCascade `TopoDS_Wire` / `TopoDS_Edge` / `TopoDS_Face` entities:

```
DXF/SVG entities → TopoDS_Wire (open paths — for profiles)
                  → TopoDS_Face (closed paths — for pockets, facing)
```

No extrusion to 3D — depth comes from the operation parameters (`start_depth`, `final_depth`). The geometry stays 2D. The frontend shows a top-down 2D view rather than a 3D viewport.

Open paths (polylines that don't close) are valid for profile operations but can't form pockets.

---

## Key methods on PartGeometry

With B-rep, these become exact rather than approximate:

```rust
impl PartGeometry {
    /// Slice the shape at height Z — returns exact curves (lines, arcs, B-splines),
    /// not polyline approximations.
    /// Uses BRepAlgoAPI_Section (shape vs. plane intersection).
    pub fn section_at_z(&self, z: f64) -> Vec<Edge> { ... }

    /// Get all faces of a specific type
    pub fn faces_by_type(&self, surface_type: SurfaceType) -> Vec<FaceInfo> { ... }

    /// Get the face at a specific index
    pub fn face(&self, face_id: u32) -> &TopoDS_Face { ... }

    /// Tessellate for display — generates TriMesh on demand
    pub fn tessellate(&self, deflection: f64) -> TriMesh { ... }

    /// Axis-aligned bounding box
    pub fn bounding_box(&self) -> BoundingBox { ... }

    /// Get edges of a face as curves
    pub fn face_edges(&self, face_id: u32) -> Vec<EdgeInfo> { ... }
}
```

**`section_at_z`** is the big win — instead of intersecting triangles with a plane and getting polyline segments, you get exact geometry: a line stays a line, an arc stays an arc, a B-spline stays a B-spline. Toolpath generators can emit exact arcs in the NC code instead of linearized approximations.

---

## Impact on toolpath generators

### Slicer (`toolpath/slicer.rs`)

**Before**: Triangle-plane intersection → polyline chains → `geo::MultiPolygon`
**After**: `BRepAlgoAPI_Section` → exact wires/edges

The slicer still converts to `geo::MultiPolygon` for the offset/pocket algorithms (geo-clipper operates on polygons), but arcs can be preserved for the final toolpath output. The conversion path:

```
TopoDS_Shape → BRepAlgoAPI_Section at Z → TopoDS_Wire (exact)
    → convert to geo::MultiPolygon (for offset operations — arcs tessellated)
    → toolpath generator works on polygons
    → arc fitting on output: detect arc-like sequences and emit ArcCw/ArcCcw segments
```

Alternatively, the offset library itself could work on exact curves. But geo-clipper/Clipper2 is polygon-based, so some tessellation at the offset stage is probably unavoidable.

**Decision needed — arc preservation strategy**:
> _Option 1: Tessellate at the slicer stage, use polygons everywhere, arc-fit at the end (simpler). Option 2: Keep exact curves through the offset stage using OpenCascade's BRepOffsetAPI (more correct but more complex). Option 3: Hybrid — use exact curves for profiles (no offset needed in controller mode), polygon offset for pockets._

### Profile (`toolpath/profile.rs`)

Big improvement. In controller compensation mode, the profile generator emits the actual part contour. With B-rep, these are exact edges — arcs are real arcs, not linearized approximations. This matters for dimensional accuracy on the machine.

### Pocket (`toolpath/pocket.rs`)

Pocket clearing still uses polygon offset (contour-parallel or zigzag). The input contour is more accurate (from exact section rather than mesh slice), but the offset passes are still polygon-based.

### Facing (`toolpath/facing.rs`)

Minimal change — facing works on the stock bounding box, not part geometry.

### Drill (`toolpath/drill.rs`)

Improvement — cylindrical faces in the B-rep directly identify holes. No need to detect circles in a tessellated cross-section. `faces_by_type(Cylinder)` gives you all holes with exact center, radius, and depth.

---

## Impact on mesh API endpoint

`GET /api/project/parts/{id}/mesh` now calls `tessellate()` on the B-rep shape instead of returning a stored TriMesh. The tessellation quality (deflection parameter) could be configurable:

```
GET /api/project/parts/{id}/mesh?deflection=0.1
```

Lower deflection = finer mesh = more triangles = better visual quality but larger response.

The response format stays the same (vertices/normals/indices for Three.js BufferGeometry).

**New**: The mesh can include face group IDs per triangle, so the frontend knows which B-rep face each triangle belongs to. This enables face highlighting on hover/click.

```json
{
  "vertices": [...],
  "normals": [...],
  "indices": [...],
  "face_ids": [0, 0, 0, 1, 1, 1, 2, 2, ...]  // one per triangle
}
```

**Decision needed — face IDs in mesh**:
> _Include face_ids in the mesh response? This enables face selection in the frontend but adds data. Could be optional (requested via query parameter)._

---

## Impact on Cargo.toml

```toml
# Before
[features]
step = ["dep:opencascade-rs"]

[dependencies.opencascade-rs]
version = "0.2"
optional = true

# After
[dependencies]
opencascade-rs = "0.2"    # Required — B-rep geometry kernel
# Remove: stl_io (OpenCascade reads STL)
# Remove: parry3d (OpenCascade handles mesh operations)
# Keep: geo, geo-clipper (for polygon offset in toolpath generators)
# Keep: nalgebra (for transform math)
```

OpenCascade can read STL directly (`RWStl::ReadFile`), so `stl_io` may be redundant. Similarly, `parry3d` was used for mesh slicing which is now handled by `BRepAlgoAPI_Section`.

**Decision needed — dependency cleanup**:
> _Drop stl_io and parry3d entirely? Or keep them as lightweight fallbacks?_

---

## Impact on project file (.camproj)

The B-rep shape (`TopoDS_Shape`) is not directly JSON-serializable. Options:

- **A**: Store the original source file path. Re-import on project load. (Current approach — works, but slower load times for large STEP files)
- **B**: Serialize the shape to BREP format (OpenCascade's native text format) and embed it in the project file as a string field.
- **C**: Save the shape as a separate `.brep` file alongside the `.camproj`. The project file references it by path.
- **D**: Bundle everything in a ZIP (`.camproj` becomes a ZIP containing project.json + geometry files).

**Decision needed — shape persistence**:
> _A is simplest (current approach). B/C keep the geometry self-contained but increase file size. D is the cleanest long-term but more complex._

---

## Impact on part update pipeline (09-part-update.md)

B-rep makes the geometry diff much richer:
- **Face-level diffing**: Compare face counts, types, areas between old and new shape
- **Edge-level diffing**: Detect added/removed holes (cylindrical faces), changed fillets
- **Exact registration**: ICP on B-rep vertices is more reliable than on mesh vertices
- **Feature-based matching**: Match faces by type + area + adjacency rather than geometric proximity

The `ChangeType` enum and `GeometryDiff` struct get richer data to work with.

---

## Impact on face selection (deferred from API review 1.7)

With B-rep, face selection becomes straightforward:
1. Frontend does raycasting on the tessellated mesh (Three.js)
2. Hit triangle → look up `face_ids[triangle_index]` → get B-rep face ID
3. Send face ID to backend: `POST /api/project/parts/{id}/orient` with `{"face_id": 5}`
4. Backend reads the face normal from the B-rep, computes the orientation transform

This resolves the deferred discussion item cleanly — no need for face clustering heuristics on triangle soup.

---

## Decisions

| # | Question | Decision |
|---|----------|----------|
| 1 | FaceInfo granularity | **Minimal** (id, type, normal, area). Richer data computed on demand from the shape. |
| 2 | STL import strategy | **C: Degraded but functional.** Sew into B-rep shell, accept triangular faces. Nudge users toward STEP. |
| 3 | Project types | **Separate 3D and 2.5D project types.** 3D projects use STEP/STL with B-rep, 3D viewport, face selection. 2.5D projects use DXF/SVG with 2D wire/profile geometry, top-down 2D view, depth from operations. Split is at the project level — a project is one or the other. |
| 4 | Arc preservation in toolpaths | **Hybrid.** Profiles in controller mode preserve exact arcs from B-rep. Pockets use polygon offset (tessellated) with arc fitting on output. |
| 5 | Face IDs in mesh response | **Always included.** One u32 per triangle — negligible overhead, enables face selection without a second request. |
| 6 | Dependency cleanup | **Drop stl_io and parry3d.** OpenCascade handles STL reading and sectioning. |
| 7 | Shape persistence in .camproj | **Separate .brep file per part** alongside the .camproj. Project JSON references by filename. OpenCascade reads/writes BREP natively. |
