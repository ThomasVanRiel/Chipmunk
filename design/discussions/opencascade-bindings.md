# OpenCascade Rust Bindings — Assessment

**Decision**: Use `opencascade-rs` (bschwind/opencascade-rs) as the OCCT binding. No architecture changes needed.

---

## Crate ecosystem

| Crate | Version | Role |
|-------|---------|------|
| `occt-sys` | 0.6.0 | Bundles and compiles OCCT 7.8.1 from source |
| `opencascade-sys` | 0.2.0 | cxx.rs bridge declarations (low-level) |
| `opencascade` | 0.2.0 | High-level ergonomic Rust API |

License: LGPL-2.1 (matches OCCT itself). FFI via cxx.rs — manual wrapping, type-safe.

## Coverage for CAM needs

**Covered:**
- STEP import/export (STEPControl_Reader/Writer)
- B-rep sectioning (BRepAlgoAPI_Section)
- Tessellation (BRepMesh_IncrementalMesh)
- Full TopoDS hierarchy + topology traversal (TopExp_Explorer)
- Wire offset (BRepOffsetAPI_MakeOffset)
- Boolean operations (fuse, cut, common)
- BRep file persistence (text + binary)
- Bounding box, mass properties, surface normals

**Not covered (minor — workarounds straightforward):**
- `BRepBuilderAPI_Sewing` — needed for STL→B-rep shell. Add the cxx binding ourselves.
- STL import — use `stl_io` crate to parse, then construct via existing builders.
- DXF import — use `dxf` crate, build OCCT wires/faces from parsed entities. OCCT itself has no DXF support.
- SVG import — use `usvg` crate, convert paths to OCCT wires. Same situation as DXF.

## Build

OCCT compiled from source on first `cargo build` (~15-30 min). Cached in CI. Requires C++ compiler + CMake. Alternative: `--no-default-features` for system OCCT.

## Alternatives considered

- **chijin** (v0.3.4, MIT) — too small API surface, missing sectioning and wire offset.
- **truck** (pure Rust B-rep) — no STEP import. Dealbreaker.
- No other significant OCCT wrappers in Rust.
