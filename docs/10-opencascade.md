# OpenCascade Interactions & Bindings

## Crate Stack

Three crates compose the OCCT bridge, all from the `bschwind/opencascade-rs` repository:

| Crate | Version | Role |
|-------|---------|------|
| `occt-sys` | 0.6.0 | Bundles OCCT 7.8.1, compiles from source on first build |
| `opencascade-sys` | 0.2.0 | Low-level cxx.rs bridge declarations |
| `opencascade` | 0.2.0 | High-level ergonomic Rust API |

License: LGPL-2.1. FFI via cxx.rs — type-safe, no raw pointers in Rust code.

**First build time**: 15–30 min (OCCT compiled from source). Subsequent builds use the compiled artifact. CI should cache the `target/` directory or use `--no-default-features` with a system OCCT install.

---

## OCCT API Usage by Module

This section maps each CAMproject module to the specific OCCT classes it calls.

### `io/step_reader.rs` — STEP Import

```
STEPControl_Reader::ReadFile()       → load .step file
STEPControl_Reader::TransferRoots() → convert STEP entities to B-rep
STEPControl_Reader::OneShape()      → extract combined TopoDS_Shape
```

The result is a `TopoDS_Shape` (typically a compound or solid). The opencascade-rs crate wraps `STEPControl_Reader` — this is **covered**.

### `io/step_reader.rs` — STEP Export (Phase 3+)

```
STEPControl_Writer::Transfer()   → add shape to writer
STEPControl_Writer::Write()      → write to file
```

Also **covered** by the crate.

### `io/brep_io.rs` — BRep Persistence

```
BRepTools::Write()    → serialize TopoDS_Shape to text .brep
BRepTools::Read()     → deserialize .brep back to TopoDS_Shape
BRep_Builder          → required as parameter to Read()
```

Both directions are **covered** by the crate. BRep is the project's native persistence format for geometry — shapes are saved as `.brep` alongside the `.camproj` JSON.

### `io/stl_reader.rs` — STL Import

OCCT itself does not parse STL reliably. The plan:

1. Parse `.stl` with the `stl_io` Rust crate → triangle soup (`Vec<Triangle>`)
2. Build OCCT faces from triangles using `BRep_Builder` + `Poly_Triangulation`
3. Sew the shell with `BRepBuilderAPI_Sewing` → watertight `TopoDS_Shell`
4. Optionally `BRepBuilderAPI_MakeSolid` if the shell is closed

`BRepBuilderAPI_Sewing` is **not covered** — requires a custom cxx.rs binding (see [Custom Bindings](#custom-bindings) below).

### `io/dxf_reader.rs` — DXF Import

OCCT has no DXF parser. The plan:

1. Parse with the `dxf` Rust crate → entities (Line, Arc, Polyline, Circle, Spline)
2. Build OCCT geometry for each entity:

```
Line      → GC_MakeSegment(P1, P2)       → Geom_TrimmedCurve → BRepBuilderAPI_MakeEdge
Arc       → GC_MakeArcOfCircle(P1,P2,P3) → Geom_TrimmedCurve → BRepBuilderAPI_MakeEdge
Circle    → gp_Circ + BRepBuilderAPI_MakeEdge(circle, u1, u2)
Polyline  → BRepBuilderAPI_MakePolygon   (sequence of points)
Spline    → Geom_BSplineCurve + BRepBuilderAPI_MakeEdge
```

3. Chain edges into wires: `BRepBuilderAPI_MakeWire::Add(edge)`
4. Close detection: if wire endpoints match within tolerance → closed
5. Closed wires → face: `BRepBuilderAPI_MakeFace(wire, planar=true)`

These geometry builders (`GC_MakeSegment`, `GC_MakeArcOfCircle`, `BRepBuilderAPI_MakeEdge`, `BRepBuilderAPI_MakeWire`, `BRepBuilderAPI_MakeFace`) need to be checked against the crate's current coverage and bound if missing (likely partially missing).

### `io/svg_reader.rs` — SVG Import

1. Parse with `usvg` → paths (MoveTo, LineTo, CurveTo, ArcTo, ClosePath)
2. Convert SVG path commands to OCCT wires using the same builders as DXF
3. SVG cubic Bézier (`CurveTo`) → `Geom_BezierCurve` or approximated as `Geom_BSplineCurve`

### `core/geometry.rs` — Shape Introspection

```
TopExp_Explorer              → traverse faces, edges, vertices of a shape
BRep_Tool::Surface(face)     → get underlying Geom_Surface for a face
GeomLib_IsPlanarSurface      → test if surface is planar, get normal
BRepGProp_Face               → compute face normal at a point
BRepBndLib::Add()            → compute bounding box into Bnd_Box
GProp_GProps + BRepGProp     → mass properties (area, volume, centroid)
BRepAdaptor_Surface          → surface type enum (Plane, Cylinder, Cone, ...)
```

Used to populate `FaceInfo` (surface type, normal, area, bounding box) on import. Most of this is **covered** by the crate (topology traversal, bounding box, mass props, surface normals).

Surface type classification uses `BRepAdaptor_Surface::GetType()` which returns `GeomAbs_SurfaceType`:

```
GeomAbs_Plane      → SurfaceType::Plane
GeomAbs_Cylinder   → SurfaceType::Cylinder
GeomAbs_Cone       → SurfaceType::Cone
GeomAbs_Sphere     → SurfaceType::Sphere
GeomAbs_Torus      → SurfaceType::Torus
GeomAbs_BSplineSurface → SurfaceType::BSpline
_                  → SurfaceType::Other
```

### `core/geometry.rs` — Tessellation

```
BRepMesh_IncrementalMesh(shape, deflection)   → compute mesh
TopExp_Explorer(shape, TopAbs_FACE)           → iterate faces
BRep_Tool::Triangulation(face, location)      → get Poly_Triangulation for a face
poly_triangulation.Node(i)                    → vertex gp_Pnt
poly_triangulation.Triangle(i)                → triangle (n1, n2, n3) indices
poly_triangulation.Normal(i)                  → per-node normal (if computed)
location.IsIdentity() / location.Transformation() → apply face transform
```

`BRepMesh_IncrementalMesh` is **covered**. `BRep_Tool::Triangulation` and `Poly_Triangulation` access may need custom bindings — check crate coverage.

Face index tracking for frontend selection: as faces are iterated with `TopExp_Explorer`, assign a sequential `face_id`. The same iteration order must be used consistently (since `TopExp_Explorer` order is deterministic for a given shape). Store the `face_id → TopAbs_Shape` mapping in `PartGeometry::faces`.

### `core/geometry.rs` — Transforms

```
gp_Trsf                     → transformation matrix (translation, rotation, scale)
BRepBuilderAPI_Transform    → apply gp_Trsf to a shape → new TopoDS_Shape
```

Used when the user repositions a part in the workspace.

### `toolpath/slicer.rs` — B-Rep Sectioning

```
gp_Pln(gp_Pnt(0,0,z), gp_Dir(0,0,1))   → horizontal plane at Z
BRepBuilderAPI_MakeFace(plane, ...)      → infinite plane shape
BRepAlgoAPI_Section(shape, plane)        → compute intersection
BRepAlgoAPI_Section::Shape()             → result shape (edges on the plane)
TopExp_Explorer(result, TopAbs_EDGE)     → iterate result edges
BRepAdaptor_Curve(edge)                  → adapt edge as a curve
BRepAdaptor_Curve::GetType()             → GeomAbs_CurveType (Line, Circle, ...)
BRepAdaptor_Curve::Circle()              → gp_Circ for arc edges
BRepAdaptor_Curve::Line()                → gp_Lin for line edges
BRepAdaptor_Curve::FirstParameter/LastParameter → edge parameter range
```

`BRepAlgoAPI_Section` is **covered**. `BRepAdaptor_Curve` with `GetType()` / `Circle()` needs to be verified — if missing, add custom bindings (needed for arc-preserving profile output in controller compensation mode).

The arc-preserving path: when a section edge is `GeomAbs_Circle`, extract the circle's center and radius, then compute the arc's start/end angles from `FirstParameter`/`LastParameter`. These map directly to `ToolpathSegment::ArcCw`/`ArcCcw` with `(i, j)` offsets.

For polygon offset operations (pocketing), arcs are tessellated at this stage using `GCPnts_TangentialDeflection` or simply `BRepAdaptor_Curve::Value(t)` at uniform parameter steps.

### `toolpath/offset.rs` — Wire Offset

```
BRepOffsetAPI_MakeOffset(wire_or_face, offset_distance)
BRepOffsetAPI_MakeOffset::Shape()  → offset result TopoDS_Shape
```

**Covered**. However, the toolpath module uses `geo-clipper` (Clipper2) for polygon offsetting rather than OCCT's wire offset, because:
- Clipper2 handles complex topologies (self-intersecting offsets, islands) more robustly
- It's already in the `geo` ecosystem that the rest of the toolpath code uses
- `BRepOffsetAPI_MakeOffset` is available as a fallback or alternative

`BRepOffsetAPI_MakeOffset` may still be used for the exact wire offset in controller compensation mode (where arc preservation matters).

---

## Custom Bindings

For OCCT classes not yet wrapped by `opencascade-rs`, we write additional cxx.rs bindings in `src/occt_ext/`.

### Directory Structure

```
src/
└── occt_ext/
    ├── mod.rs           # re-exports all custom bindings
    ├── sewing.rs        # BRepBuilderAPI_Sewing
    ├── curve_adaptor.rs # BRepAdaptor_Curve (GetType, Circle, Line, ...)
    ├── triangulation.rs # BRep_Tool::Triangulation, Poly_Triangulation
    └── builders.rs      # GC_MakeSegment, GC_MakeArcOfCircle, BRepBuilderAPI_MakeEdge, etc.
build.rs                 # cxx build config
```

### cxx.rs Binding Pattern

Each binding file declares an `#[cxx::bridge]` with the C++ types and functions needed, plus a `.cpp` file that implements thin wrappers where needed (e.g., to catch OCCT exceptions).

**Example: `BRepBuilderAPI_Sewing`**

```rust
// src/occt_ext/sewing.rs
#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("occt_ext/sewing.h");

        type BRepBuilderAPI_Sewing;

        fn new_sewing(tolerance: f64) -> UniquePtr<BRepBuilderAPI_Sewing>;
        fn sewing_add(sewing: Pin<&mut BRepBuilderAPI_Sewing>, shape: &TopoDS_Shape);
        fn sewing_perform(sewing: Pin<&mut BRepBuilderAPI_Sewing>);
        fn sewing_sewn_shape(sewing: &BRepBuilderAPI_Sewing) -> UniquePtr<TopoDS_Shape>;
    }
}
```

```cpp
// src/occt_ext/sewing.cpp  (thin C++ wrapper)
#include <BRepBuilderAPI_Sewing.hxx>
#include <TopoDS_Shape.hxx>

std::unique_ptr<BRepBuilderAPI_Sewing> new_sewing(double tolerance) {
    return std::make_unique<BRepBuilderAPI_Sewing>(tolerance);
}
void sewing_add(BRepBuilderAPI_Sewing& s, const TopoDS_Shape& shape) {
    s.Add(shape);
}
void sewing_perform(BRepBuilderAPI_Sewing& s) {
    s.Perform();
}
std::unique_ptr<TopoDS_Shape> sewing_sewn_shape(const BRepBuilderAPI_Sewing& s) {
    return std::make_unique<TopoDS_Shape>(s.SewedShape());
}
```

**Exception handling**: OCCT throws C++ exceptions (`Standard_Failure`). All custom `.cpp` wrappers must catch these and convert them to a return-value error or a `Result<_, String>` on the Rust side:

```cpp
std::unique_ptr<TopoDS_Shape> sewing_sewn_shape_safe(
    const BRepBuilderAPI_Sewing& s, rust::String& err_out) {
    try {
        return std::make_unique<TopoDS_Shape>(s.SewedShape());
    } catch (Standard_Failure& e) {
        err_out = e.GetMessageString();
        return nullptr;
    }
}
```

On the Rust side, check the returned pointer and the `err_out` string to convert to `Result`.

### `build.rs` Configuration

```rust
// build.rs
fn main() {
    cxx_build::bridges([
        "src/occt_ext/sewing.rs",
        "src/occt_ext/curve_adaptor.rs",
        "src/occt_ext/triangulation.rs",
        "src/occt_ext/builders.rs",
    ])
    .include("src")
    .include(occt_include_dir())   // path to OCCT headers from occt-sys
    .compile("occt_ext");

    // Link against the OCCT libraries already built by occt-sys
    for lib in OCCT_LIBS {
        println!("cargo:rustc-link-lib={lib}");
    }
}
```

---

## Thread Safety

OCCT is **not thread-safe** in general:
- `TopoDS_Shape` objects use reference counting internally (OCCT handles). A `TopoDS_Shape` can be safely read from multiple threads if not mutated. Mutation (tessellation, boolean ops, section) is **not thread-safe**.
- OCCT has a thread-local tolerance context — no global state issues there.

**Consequence for Axum**: All OCCT computation (import, tessellation, sectioning, toolpath generation) must run in `tokio::task::spawn_blocking`. Never call OCCT functions directly in an async context.

```rust
// In API handler:
let shape = tokio::task::spawn_blocking(move || {
    import_step(&file_bytes)  // OCCT call — runs on blocking thread pool
}).await??;
```

---

## Error Handling Strategy

OCCT operations use a builder pattern with `IsDone()` / `Error()` checks:

```rust
// Pattern for wrapping builder results
pub fn section_at_z(shape: &TopoDS_Shape, z: f64) -> Result<Vec<TopoDS_Wire>, OcctError> {
    let result = BRepAlgoAPI_Section::new(shape, &make_plane_at_z(z));
    if !result.is_done() {
        return Err(OcctError::SectionFailed);
    }
    Ok(collect_wires(&result.shape()))
}
```

Define a project-level `OcctError` enum in `src/core/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum OcctError {
    #[error("STEP import failed: {0}")]
    StepImport(String),
    #[error("B-rep section failed at Z={0}")]
    SectionFailed(f64),
    #[error("Wire offset failed")]
    OffsetFailed,
    #[error("Tessellation failed")]
    TessellationFailed,
    #[error("Sewing failed: {0}")]
    SewingFailed(String),
    #[error("Geometry construction error: {0}")]
    Construction(String),
}
```

---

## Binding Coverage Checklist

Before implementing each module, verify which APIs are already covered by `opencascade-rs` (check `opencascade-sys/src/lib.rs` for declared bindings) and which need custom wrappers.

| OCCT Class | Module | Covered? | Notes |
|---|---|---|---|
| `STEPControl_Reader` | io/step_reader | ✓ | Full import |
| `STEPControl_Writer` | io/step_reader | ✓ | Full export |
| `BRepTools::Read/Write` | io/brep_io | ✓ | BRep persistence |
| `BRepBuilderAPI_Sewing` | io/stl_reader | ✗ | Add custom binding |
| `Poly_Triangulation` | io/stl_reader | ? | Check crate |
| `BRepMesh_IncrementalMesh` | core/geometry | ✓ | Tessellation |
| `BRep_Tool::Triangulation` | core/geometry | ? | Check crate |
| `TopExp_Explorer` | core/geometry | ✓ | Topology traversal |
| `BRepBndLib` / `Bnd_Box` | core/geometry | ✓ | Bounding box |
| `BRepGProp` / `GProp_GProps` | core/geometry | ✓ | Mass props, normals |
| `BRepAdaptor_Surface` | core/geometry | ✓ | Surface type enum |
| `gp_Trsf` / `BRepBuilderAPI_Transform` | core/geometry | ✓ | Shape transform |
| `BRepAlgoAPI_Section` | toolpath/slicer | ✓ | B-rep sectioning |
| `BRepAdaptor_Curve` | toolpath/slicer | ? | Arc type detection |
| `GCPnts_TangentialDeflection` | toolpath/slicer | ? | Arc tessellation |
| `BRepOffsetAPI_MakeOffset` | toolpath/offset | ✓ | Wire offset |
| `BRepBuilderAPI_MakeEdge` | io/dxf_reader | ? | Edge from curve |
| `BRepBuilderAPI_MakeWire` | io/dxf_reader | ? | Wire from edges |
| `BRepBuilderAPI_MakeFace` | io/dxf_reader | ? | Face from wire |
| `GC_MakeSegment` | io/dxf_reader | ? | Line geometry |
| `GC_MakeArcOfCircle` | io/dxf_reader | ? | Arc geometry |

**Convention**: `✓` = confirmed in crate, `✗` = confirmed missing, `?` = check `opencascade-sys/src/lib.rs` before implementing.

---

## Geometry Conventions

All geometry is stored and processed in **millimeters**. OCCT itself is unit-agnostic, but all values passed to and returned from OCCT APIs are in mm. Unit conversion (if the user works in inches) is applied at the API boundary in `api/routes.rs`, not inside the OCCT wrappers.

**Coordinate system**: Z+ up, Z=0 at the part's WCS origin (top face of part after orientation). OCCT works in the same right-handed coordinate system.

**Tolerance**: Use `0.001 mm` (1 micron) as the default OCCT geometric tolerance throughout (`Precision::Confusion()` returns `1e-7` in OCCT units, but we work at the 1-micron scale for CAM purposes). Pass tolerance explicitly rather than relying on OCCT defaults.

**Arc representation**: Arcs in `ToolpathSegment` use `(i, j)` — the center offset from the arc start point. This is the same convention as G-code `I J` words. OCCT gives the arc center in world coordinates (`gp_Circ::Location()`), so subtract the start point to get `(i, j)`.
