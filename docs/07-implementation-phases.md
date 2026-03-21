# Implementation Phases

Entirely CLI-driven. The complete 2.5D workflow (drilling + milling) is usable without a frontend. The browser UI and 3D project support are deferred to the backlog — the tool is useful without them.

Core philosophy:
- **Get real hardware feedback early** — Heidenhain is the target machine; it should be the first post-processor.
- **Simplest complete workflow first** — manual drilling (XY + quill confirmation) before automatic cycles, before milling.
- **One SVG + one YAML = one job** — SVG stroke colors map to operations; the YAML file configures each color. No GUI required to select contours.

---

## Phase 1: Scaffolding + SVG/DXF Import

**Goal**: Backend running, SVG and DXF import working, project save/load. Validated via health check and test suite.

### Backend Tasks (Rust)

1. **Project scaffolding**
   - `Cargo.toml` with all dependencies (clap, serde, serde_yaml, opencascade-rs, geo, geo-clipper, mlua, uuid, chrono, tracing, anyhow, thiserror)
   - `src/main.rs` — entry point, clap subcommand dispatch (`drill`, `mill`, `postprocessors`)
   - `src/lib.rs` — module declarations

2. **Core data model (minimal)**
   - `core/units.rs`: `Units` enum (Mm, Inch)
   - `core/geometry.rs`: `PartGeometry` wrapping `TopoDS_Shape`, `BoundingBox`
   - `core/project.rs`: `Project` struct, `ProjectType::TwoHalfD`

3. **SVG import** — primary input format
   - `io/svg_reader.rs`: Parse SVG paths into `TopoDS_Wire` / `TopoDS_Face`
   - **Preserve stroke color per entity** — stored alongside geometry for operation mapping
   - Circles → `TopoDS_Vertex` at center + radius metadata (for drill point extraction)
   - Closed paths → faces; open paths → wires

4. **DXF import** — secondary input format
   - `io/dxf_reader.rs`: Parse DXF via OpenCascade → wires/faces
   - Preserve entity color (ACI or RGB) per entity

5. **B-rep persistence**
   - `io/brep_io.rs`: save/load `.brep`
   - `io/project_file.rs`: `.camproj` JSON

### Tests

6. `test_svg_reader.rs` — circle, rectangle, open path; stroke color preserved
   `test_dxf_reader.rs` — lines, arcs, circles; entity color preserved
   `test_brep_io.rs` — save + reload roundtrip

### Deliverable

SVG and DXF import working — color groups extracted correctly. All import and roundtrip tests pass.

---

## Phase 2: Manual Drill + Heidenhain Export (CLI)

**Goal**: First complete end-to-end workflow via CLI. SVG circles → drill points → Heidenhain NC → run on machine.

Target output (three holes, operator runs in single block mode):
```
BEGIN PGM DRILL MM
BLK FORM 0.1 Z X+0.000 Y+0.000 Z-50.000
BLK FORM 0.2 X+100.000 Y+100.000 Z+0.000
TOOL CALL 1 Z S0
L Z+5.000 FMAX
L X+25.000 Y+15.000 FMAX
L X+75.000 Y+15.000 FMAX
L X+75.000 Y+65.000 FMAX
L Z+50.000 FMAX
END PGM DRILL MM
```

No STOP blocks, no spindle commands — the operator activates single block mode on the controller and drills by hand between positions.

### Backend Tasks

1. **Core types** — `Tool`, `Setup` (WCS origin, clearance height), `Operation`, `DrillParams`
2. **Drill toolpath** — `DrillOperation`: extract circle centers from geometry → `Vec<DrillPoint>`
3. **`OperationType` trait** — `toolpath/registry.rs`
4. **NC IR** — `nc/ir.rs`: `RapidMove`, `ProgramStop`, `SpindleOn/Off`, `ToolChange`, `ProgramEnd`
5. **NC compiler** — `nc/compiler.rs`: tool call → spindle → clearance → points with STOP → end
6. **mlua bridge** — `nc/bridge.rs`: fresh Lua VM per call, load base.lua + post-processor, call `M.generate()`
7. **Post-processor registry** — `nc/postprocessors/mod.rs`: `BUILTIN_POSTPROCESSORS` array
8. **`postprocessors/base.lua`** — `M.fmt()`, `M.hh_coord()` (explicit sign)
9. **`postprocessors/heidenhain.lua`** — manual drill mode: header, `L X+n Y+n FMAX` + `STOP` per point, footer

### CLI

```bash
# From SVG — extract circle centers from all circles
camproject drill holes.svg --tool-number 1 --tool-name "Drill 6mm" --diameter 6 \
  --spindle-speed 1500 --clearance 5 --postprocessor heidenhain --output DRILL.H

# Filter by stroke color (for SVGs with mixed content)
camproject drill part.svg --color "#00ff00" --tool-number 1 ...

# Explicit coordinates
camproject drill --at 25,15 --at 75,15 --at 75,65 --tool-number 1 ...

# List post-processors
camproject postprocessors
```

### Tests

10. `test_drill.rs`, `test_nc_compiler.rs`
    Golden file: `tests/fixtures/nc/heidenhain_manual_drill.H`
    Fixture: `tests/fixtures/holes.svg`

### Deliverable

`camproject drill holes.svg --postprocessor heidenhain --output DRILL.H` → load on Heidenhain TNC → quill drill workflow works.

---

## Phase 3: Automatic Drill Cycles + Per-Program Export

**Goal**: Native canned cycles. Per-tool NC file export for no-tool-changer machines.

### No-Tool-Changer Workflow

Z=0 is set at the **tool tip** before each program. No tool length compensation needed:

```
Load center drill → touch off Z at tip → run T1_CENTER_DRILL.H
Load Ø6 drill    → touch off Z at tip → run T2_DRILL_6.H
```

### Backend Tasks

1. **Full DrillParams** — peck depth, chip-break, dwell, retract plane, `DrillStrategy` enum, `use_canned_cycle` flag
2. **Explicit Z move fallback** — drill toolpath always generates explicit moves; `compile_nc` hook emits cycles when supported
3. **`compile_nc` hook** — `DrillOperation::compile_nc(op, caps)` → `CycleDefine`/`CycleCall`/`CycleOff` or `None`
4. **NC IR additions** — `CycleDefine { cycle_type, params }`, `CycleCall { x, y }`, `CycleOff`
5. **`PostProcessorCapabilities`** — `get_capabilities()` reads `supported_cycles`, `optional_skip_strategy`, `tool_length_compensation` from Lua module
6. **`ToolLengthMode::ZeroAtTip`** — no G43 / no Heidenhain tool length; set per setup
7. **Heidenhain canned cycles** — CYCL DEF 200, 203, 207; `L X+n Y+n FMAX M99`
8. **G-code canned cycles** — Haas G81/G83/G84 with G90 guard
9. **Optional operations** — `optional_skip_level` (1–9); Heidenhain: `M1`; G-code: `/` prefix
10. **Per-program CLI export** — `--output-dir` writes one file per tool; `--dry-run` lists what would be generated

### CLI

```bash
camproject drill holes.svg --params drill.yaml --output-dir ./nc/
# → nc/T1_CENTER_DRILL.H, nc/T2_DRILL_6.H
```

### Tests

11. `test_drill_cycles.rs`, golden files for CYCL DEF 200/203, G83
    Per-tool split test: two-tool job → two correct NC files

### Deliverable

Two drill ops, different tools → `--output-dir nc/` → two `.H` files → load each in sequence, touching off Z at tip between tools.

---

## Phase 4: CLI 2.5D Milling — SVG Color Workflow

**Goal**: Profile, pocket, facing driven by SVG stroke colors + YAML job file. One `camproject mill` command generates all NC programs for a job.

### SVG Color Workflow

Paths in the SVG are selected by **stroke color**. The YAML maps each color to a full operation configuration. Circles → drill points; closed paths → profile or pocket; open paths with a color → ignored or error.

```yaml
# job.yaml
postprocessor: heidenhain
clearance: 5.0

operations:
  - color: "#00ff00"      # green circles → center drill
    type: drill
    tool_number: 1
    tool_name: "Center Drill"
    tool_diameter: 3.0
    spindle_speed: 2000

  - color: "#0000ff"      # blue circles → through drill
    type: drill
    tool_number: 2
    tool_name: "Drill 6mm"
    tool_diameter: 6.0
    depth: 20.0
    strategy: peck
    peck_depth: 4.0
    spindle_speed: 1500

  - color: "#ff0000"      # red closed paths → outside profile
    type: profile
    side: outside
    tool_number: 3
    tool_name: "6mm End Mill"
    tool_diameter: 6.0
    depth: 8.0
    stepdown: 2.0
    spindle_speed: 8000
    feed_rate: 800

  - color: "#ff8800"      # orange closed paths → pocket
    type: pocket
    tool_number: 3
    depth: 6.0
    stepdown: 2.0
    stepover: 2.4
    entry: helix
    helix_radius: 3.0
    spindle_speed: 8000
    feed_rate: 800
```

```bash
camproject mill part.svg --params job.yaml --output-dir ./nc/
# → nc/T1_CENTER_DRILL.H
# → nc/T2_DRILL_6MM.H
# → nc/T3_6MM_END_MILL.H   (profile + pocket combined, same tool)

camproject mill part.svg --params job.yaml --dry-run
# prints: 3 circles #00ff00 → drill T1
#         2 circles #0000ff → drill T2
#         1 path   #ff0000 → profile T3
#         2 paths  #ff8800 → pocket T3

# Roughing + finishing pass with same DXF, override allowance
camproject mill part.svg --params job.yaml --color "#ff0000" --allowance 0.2 --output rough.H
camproject mill part.svg --params job.yaml --color "#ff0000" --allowance 0.0 --output finish.H
```

### Color convention (suggested, not enforced)

| Color | Operation |
|-------|-----------|
| `#00ff00` green | Center drill / spot drill |
| `#0000ff` blue | Through drill |
| `#ff0000` red | Profile outside |
| `#ff00ff` magenta | Profile inside |
| `#ff8800` orange | Pocket |
| `#00ffff` cyan | Facing |

The mapping is entirely user-controlled via YAML — this is just a suggested starting convention.

### Backend Tasks

1. **YAML job file** — `serde_yaml`, `JobParams` with top-level fields + `operations: Vec<OperationConfig>`
2. **Color-keyed geometry grouping** — `io/svg_reader.rs` returns `Vec<ColorGroup { color: String, entities: Vec<Entity> }>`; DXF reader same
3. **`camproject mill` subcommand** — parse SVG → group by color → match each group to `OperationConfig` → run operation → collect NCBlocks by tool → write per-tool files
4. **`--dry-run` flag** — print color groups and matched operations, exit without generating NC
5. **Polygon offset** — `toolpath/offset.rs`: `offset_polygon()`, `iterative_offset()`, contour extraction from color group
6. **Depth strategy** — `toolpath/depth_strategy.rs`
7. **Facing generator** — `toolpath/facing.rs`
8. **Profile generator** — `toolpath/profile.rs`: inside/outside/on, CAM/controller comp, lead-in, tabs, depth passes
9. **Pocket generator** — `toolpath/pocket.rs`: contour-parallel/zigzag, helix/plunge/ramp entry, depth passes
10. **NC IR extensions** — `LinearMove` with feed, `ArcMove`, `SetFeedRate`, `CoolantOn/Off`, `CutterCompLeft/Right/Off`
11. **Heidenhain milling (Lua)** — `L X+n Y+n F+n`, `CC`/`C DR+/-`, `RL`/`RR`/`R0`, `M8`/`M9`
12. **Haas post-processor (Lua)** — extend with G1/G2/G3, G41/G42, M7/M8 milling support

### Tests

13. `test_offset.rs`, `test_depth_strategy.rs`, `test_facing.rs`, `test_profile.rs`, `test_pocket.rs`
    `test_color_grouping.rs`: SVG with 3 colors → 3 groups with correct entities
    Golden files: `heidenhain_profile.H`, `heidenhain_pocket.H`, `heidenhain_facing.H`
    CLI integration: `camproject mill fixture.svg --params fixture_job.yaml --dry-run` matches expected groups

### Deliverable

```bash
camproject mill part.svg --params job.yaml --output-dir ./nc/
```

One command processes the full job: center drill, through drill, profile, pocket — each tool gets its own `.H` file. Draw in Inkscape, assign stroke colors, run command, machine the part.

---

## Priority and Dependencies

```
Phase 1 (import + color parsing)
    └─→ Phase 2 (manual drill CLI)     ← first hardware test
            └─→ Phase 3 (auto cycles)  ← full drilling done
                    └─→ Phase 4 (mill CLI + SVG colors)  ← complete 2.5D workflow
```

Phases 1–4 cover the full 2.5D machining workflow for a no-tool-changer Heidenhain machine, driven entirely from the command line. See `tasks/backlog.md` for deferred items (frontend, 3D projects, Inkscape extension).

---

## Backlog (Deferred)

Items with clear design but no scheduled phase. Implement when the core CLI workflow is solid.

- **REST API** — axum server exposing the same library functions over HTTP. Peer to the CLI, not a wrapper. Required before any frontend work. Design in `docs/02-api-design.md`.
- **Browser frontend** — 2D canvas viewport, operation panels, NC preview. Design in `tasks/backlog.md`.
- **3D projects** — STEP/STL import, B-rep slicer, Three.js viewport. Most milling is 2.5D; slicing is the same pipeline regardless of input.
- **Inkscape extension** — appears under Extensions > CAM; calls `camproject` CLI; shows parameter dialog in Inkscape. Eliminates file management step.
- **Sinumerik post-processor**
- **Part update pipeline** — geometry diff, ICP registration, operation audit
- **Stock simulation** — Z-buffer material removal
- **CAD integrations** — Onshape API, FreeCAD CLI bridge, watch folder
