# Phase 1: Scaffolding + SVG/DXF Import

**Goal**: Core library running, SVG/DXF import working with stroke color preservation, project save/load. Validated via test suite.

---

## Backend

### Project scaffolding
- [ ] Create `Cargo.toml` with all dependencies (clap, serde, serde_yaml, opencascade-rs, geo, geo-clipper, mlua, uuid, chrono, tracing, anyhow, thiserror)
- [ ] `src/main.rs` — entry point, clap subcommand dispatch (`drill`, `mill`, `postprocessors`)
- [ ] `src/lib.rs` — module declarations

### Core data model
- [ ] `core/units.rs` — `Units` enum (Mm, Inch), conversion factor
- [ ] `core/geometry.rs` — `PartGeometry` wrapping `TopoDS_Shape`, `BoundingBox`, `FaceInfo`, `SurfaceType`
- [ ] `core/project.rs` — `Project` struct, `ProjectType::TwoHalfD`, `ProjectMetadata`

### File I/O
- [ ] `io/svg_reader.rs` — SVG → `Vec<ColorGroup { color: String, entities: Vec<SvgEntity> }>`
  - `SvgEntity` enum: `Circle { center: Point2, radius: f64 }`, `ClosedPath(Polygon)`, `OpenPath(LineString)`
  - Circles are geometry-agnostic at import time — whether they become drill points or circular pockets is determined by the YAML operation config in Phase 4
  - Normalize stroke colors to lowercase `#rrggbb`; handle named colors (`red`) and `rgb(r,g,b)` syntax
  - **Stroke color is the key discriminator** — fill color is ignored
- [ ] `io/dxf_reader.rs` — DXF → same `Vec<ColorGroup>` structure; ACI color index or RGB → hex
- [ ] `io/brep_io.rs` — save/load `.brep` (OpenCascade native)
- [ ] `io/project_file.rs` — `.camproj` JSON save/load with `brep_file` references

---

## Tests

- [ ] `tests/test_geometry.rs` — `PartGeometry` creation, bounding box, face enumeration
- [ ] `tests/test_dxf_reader.rs` — rectangle DXF → expected wire, circle DXF → expected face
- [ ] `tests/test_svg_reader.rs` — circle with stroke `#ff0000` → `ColorGroup { color: "#ff0000", entities: [Circle] }`; closed path → `ClosedPath`; color normalization (`red`, `rgb(255,0,0)`, `#FF0000` all → `#ff0000`)
- [ ] `tests/test_brep_io.rs` — save + reload `.brep` roundtrip preserves geometry
- [ ] Add fixtures: `tests/fixtures/rectangle.dxf`, `circle.svg`, `profile_with_arcs.dxf`

---

## Deliverable

SVG and DXF import working — color groups extracted correctly. All import and roundtrip tests pass.
