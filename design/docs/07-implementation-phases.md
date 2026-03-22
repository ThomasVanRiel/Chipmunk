# Implementation Phases

Entirely CLI-driven. The complete 2.5D workflow (drilling + milling) is usable without a frontend. The browser UI and 3D project support are deferred to the backlog — the tool is useful without them.

Core philosophy:
- **Get real hardware feedback early** — Heidenhain is the target machine; it should be the first post-processor.
- **Simplest complete workflow first** — manual drilling with explicit points before geometry import, before automatic cycles, before milling.
- **No heavy dependencies until needed** — OpenCascade is deferred until geometry import requires it.

---

## Phase 1: Scaffolding + Manual Drill (Points in YAML)

**Goal**: First complete end-to-end workflow via CLI. Explicit XY points in YAML → manual drill toolpath → Heidenhain NC → run on machine.

Target output (three holes, operator runs in single block mode):
```
BEGIN PGM DRILL MM
TOOL CALL 1 Z S800
; ENABLE SINGLE BLOCK MODE FOR MANUAL DRILLING
M0
L Z+5.000 FMAX M3
L X+25.000 Y+15.000 FMAX
L X+75.000 Y+15.000 FMAX
L X+75.000 Y+65.000 FMAX
L Z+5.000 FMAX M5
END PGM DRILL MM
```

### Backend Tasks

1. **Project scaffolding**
   - `Cargo.toml` with dependencies (clap, serde, serde_yaml, mlua, tracing, anyhow, thiserror)
   - `src/main.rs` — entry point, clap dispatch
   - `src/lib.rs` — module declarations
   - Module structure: `core/`, `toolpath/`, `nc/`, `io/`

2. **Core types (minimal)**
   - `core/tool.rs`: `Tool` (number, name, diameter, spindle_speed)
   - `core/operation.rs`: `Operation`, `DrillParams`, `DrillStrategy::Manual`
   - `core/units.rs`: `Units` enum (Mm, Inch)

3. **YAML job parsing**
   - `io/job.rs`: parse YAML job file into `JobConfig`
   - No `geometry:` field required — operations use `points:` for explicit coordinates
   - Missing required fields → hard error, exit 1

4. **Manual drill toolpath**
   - `toolpath/drill.rs`: `DrillOperation` — takes `Vec<[f64; 2]>` points → `Vec<ToolpathSegment>`
   - Segments: `Rapid` moves only (no Z motion for manual strategy)

5. **NC IR (minimal)**
   - `nc/ir.rs`: `NCBlock` enum — `Comment`, `Stop`, `SpindleOn`, `SpindleOff`, `ToolChange`, `Rapid`, `ProgramEnd`

6. **NC compiler**
   - `nc/compiler.rs`: tool call → comment + M0 acknowledge → spindle on → clearance → rapid to each point → retract → spindle off → end

7. **mlua bridge**
   - `nc/bridge.rs`: fresh Lua VM per call, load `base.lua` + post-processor, call `M.generate()`

8. **Post-processor registry**
   - `nc/postprocessors/mod.rs`: `BUILTIN_POSTPROCESSORS` array, embedded via `include_str!()`

9. **`postprocessors/base.lua`** — shared helpers: `M.fmt()`, `M.hh_coord()` (explicit sign formatting)

10. **`postprocessors/heidenhain.lua`** — manual drill: header (`BEGIN PGM`), tool call, comment + M0, spindle on, `L X+n Y+n FMAX` per point, spindle off, footer (`END PGM`)

### CLI

```bash
chipmunk drill.yaml --output DRILL.H
chipmunk drill.yaml                     # → stdout
chipmunk postprocessors                 # list available post-processors
```

Minimal `drill.yaml`:

```yaml
postprocessor: heidenhain
clearance: 5.0

operations:
  - type: drill
    strategy: manual
    tool_number: 1
    tool_name: "Drill 6mm"
    tool_diameter: 6.0
    spindle_speed: 800
    points:
      - [25.0, 15.0]
      - [75.0, 15.0]
      - [75.0, 65.0]
```

### Tests

11. `test_drill.rs` — manual drill points → correct toolpath segments
12. `test_nc_compiler.rs` — toolpath → correct IR blocks (comment, stop, spindle, rapids)
13. Golden file: `tests/fixtures/nc/heidenhain_manual_drill.H`
    Fixture: `tests/fixtures/drill.yaml`

### Deliverable

`chipmunk drill.yaml --output DRILL.H` → load on Heidenhain TNC → quill drill workflow works. First hardware test.

---

## Phase 2: Automatic Drill Cycles

**Goal**: Full drilling capability — peck, chip break, bore, tap. Native canned cycles for Heidenhain and Haas. Still points-based (no geometry import yet).

### No-Tool-Changer Workflow

Z=0 is set at the **tool tip** before each program. No tool length compensation needed:

```
Load center drill → touch off Z at tip → run T1_CENTER_DRILL.H
Load Ø6 drill    → touch off Z at tip → run T2_DRILL_6.H
```

### Backend Tasks

1. **Full DrillParams** — `depth`, `peck_depth`, `chip_break_retract`, `dwell`, `retract_plane`, `DrillStrategy` enum (manual/simple/peck/chip_break/bore/tap), `use_canned_cycle` flag
2. **Explicit Z move fallback** — drill toolpath always generates explicit moves; NC compiler emits cycles when post-processor supports them
3. **NC IR additions** — `Linear`, `CycleDefine { cycle_type, params }`, `CycleCall { x, y }`, `CycleOff`
4. **Drill point patterns** — `points:` accepts `circle_pattern` (center, radius, count, optional start_angle), `line_pattern` (start, end, count or spacing), `rect_pattern` (corner, spacing, count, optional angle) alongside explicit `[x, y]` coordinates. Patterns are preserved through the IR so post-processors can emit native pattern support (e.g. Heidenhain `PATTERN DEF`). `base.lua` provides `M.expand_patterns(blocks)` for post-processors without native support.
5. **`PostProcessorCapabilities`** — `get_capabilities()` reads `supported_cycles`, `optional_skip_strategy` from Lua module
6. **`ToolLengthMode::ZeroAtTip`** — no G43 / no Heidenhain tool length; set per setup
7. **Heidenhain canned cycles** — CYCL DEF 200, 203, 207; `L X+n Y+n FMAX M99`; `PATTERN DEF` for circle/line/rect patterns
8. **Haas post-processor** — G81/G83/G84 with G90 guard; uses `M.expand_patterns()` for pattern fallback
9. **Optional operations** — `optional_skip_level` (1–9); Heidenhain: `M1`; G-code: `/` prefix
10. **`--tool` filter** — output only operations for a specific tool number
11. **`--check` flag** — validate job file, print summary, exit without generating NC

### CLI

```bash
chipmunk drill.yaml --output DRILL.H
chipmunk drill.yaml --tool 1 --output T1_CENTER_DRILL.H
chipmunk drill.yaml --tool 2 --output T2_DRILL_6.H
chipmunk drill.yaml --check
```

### Tests

12. `test_drill_cycles.rs` — golden files for CYCL DEF 200/203, G83
13. Multi-tool output test: two-tool job → combined NC file with correct tool changes
14. `--tool` filter test: two-tool job → single-tool output
15. `test_patterns.rs` — circle, line, rect patterns produce correct point positions; Heidenhain emits native `PATTERN DEF`; Haas expands to individual points

### Deliverable

Two drill ops → single `.H` file (stdout or `--output`) → load on Heidenhain TNC → canned cycle drill workflow works. Full drilling without geometry import.

---

## Phase 3: SVG/DXF Import + Color Workflow

**Goal**: Geometry-driven operations. SVG/DXF files provide drill points and contours via stroke color mapping.

### Backend Tasks

1. **SVG import** — `io/svg_reader.rs`
   - Parse SVG paths, preserve stroke color per entity
   - Circles → center point + radius metadata (for drill point extraction)
   - Closed paths → contours; open paths → wires
   - Returns `Vec<ColorGroup { color: String, entities: Vec<Entity> }>`

2. **DXF import** — `io/dxf_reader.rs`
   - Parse DXF entities, preserve entity color (ACI or RGB)
   - Same `ColorGroup` output as SVG

3. **Color-keyed geometry grouping** — match each color group to an `OperationConfig` by `color:` field
   - Unmatched colors in geometry → **hard error**, exit 1

4. **WCS marker** — `wcs_marker_color:` in YAML identifies a circle whose center defines the WCS origin

5. **YAML `geometry:` field** — path to SVG/DXF file (relative to YAML); operations use `color:` instead of `points:`

6. **`--geometry` override flag** — override the geometry path declared in the YAML

7. **`--color` filter** — process only operations matching a specific stroke color

8. **`--plot` flag** — generate SVG of toolpaths alongside NC output. Separate layers for geometry, stock, and toolpaths. Color coded by operation; dashed rapids, solid feeds. Respects `--tool` and `--color` filters.

### CLI

```bash
chipmunk job.yaml --output DRILL.H
chipmunk job.yaml --check
chipmunk job.yaml --geometry revised_part.svg --output part.H
chipmunk job.yaml --color "#0000ff" --output holes_only.H
```

### Tests

8. `test_svg_reader.rs` — circle, rectangle, open path; stroke color preserved
9. `test_dxf_reader.rs` — lines, arcs, circles; entity color preserved
10. `test_color_grouping.rs` — SVG with 3 colors → 3 groups with correct entities
11. Color mismatch test — unmatched color → hard error

### Deliverable

`chipmunk job.yaml --check` → color groups parsed and matched. Drill operations work with both `points:` (from Phase 1) and `color:` (geometry-driven). Ready for milling.

---

## Phase 4: 2.5D Milling

**Goal**: Profile, pocket, facing driven by SVG stroke colors + YAML job file. One command generates NC output for the full job.

### Backend Tasks

1. **Polygon offset** — `toolpath/offset.rs`: `offset_polygon()`, `iterative_offset()`, contour extraction from color group
2. **Depth strategy** — `toolpath/depth_strategy.rs`
3. **Facing generator** — `toolpath/facing.rs`
4. **Profile generator** — `toolpath/profile.rs`: inside/outside/on, CAM/controller comp, lead-in, tabs, depth passes
5. **Pocket generator** — `toolpath/pocket.rs`: contour-parallel/zigzag, helix/plunge/ramp entry, depth passes
6. **NC IR extensions** — `Linear` with feed, `ArcCw`/`ArcCcw`, `SetFeedRate`, `CoolantOn/Off`, `CompLeft/CompRight/CompOff`
7. **Heidenhain milling (Lua)** — `L X+n Y+n F+n`, `CC`/`C DR+/-`, `RL`/`RR`/`R0`, `M8`/`M9`
8. **Haas milling (Lua)** — G1/G2/G3, G41/G42, M7/M8
9. **`--allowance` override flag** — override `allowance:` on all matching operations (roughing/finishing workflow)

### CLI

```bash
chipmunk job.yaml --output part.H
chipmunk job.yaml --check

# Roughing + finishing pass, override allowance per run
chipmunk job.yaml --color "#ff0000" --allowance 0.2 --output rough.H
chipmunk job.yaml --color "#ff0000" --allowance 0.0 --output finish.H
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

### Tests

10. `test_offset.rs`, `test_depth_strategy.rs`, `test_facing.rs`, `test_profile.rs`, `test_pocket.rs`
11. Golden files: `heidenhain_profile.H`, `heidenhain_pocket.H`, `heidenhain_facing.H`
12. CLI integration: `chipmunk fixture_job.yaml --check` matches expected groups

### Deliverable

```bash
chipmunk job.yaml --output part.H
```

One command processes the full job: center drill, through drill, profile, pocket → single NC file to stdout or `--output`. Draw in Inkscape, assign stroke colors, run command, machine the part.

---

## Priority and Dependencies

```
Phase 1 (scaffolding + manual drill, points)  ← first hardware test
    └─→ Phase 2 (automatic drill cycles)      ← full drilling done
            └─→ Phase 3 (SVG/DXF import)      ← geometry-driven ops
                    └─→ Phase 4 (2.5D milling) ← complete workflow
```

Phases 1–4 cover the full 2.5D machining workflow for a no-tool-changer Heidenhain machine, driven entirely from the command line. See `tasks/backlog.md` for deferred items (frontend, 3D projects, Inkscape extension).

---

## Backlog (Deferred)

Items with clear design but no scheduled phase. Implement when the core CLI workflow is solid.

- **REST API** — axum server exposing the same library functions over HTTP. Peer to the CLI, not a wrapper. Required before any frontend work. Design in `design/docs/deferred/02-api-design.md`.
- **Browser frontend** — 2D canvas viewport, operation panels, NC preview.
- **3D projects** — STEP/STL import, B-rep geometry (OpenCascade via opencascade-rs), 3D viewport.
- **Inkscape extension** — appears under Extensions > CAM; calls `chipmunk` CLI; shows parameter dialog in Inkscape.
- **Sinumerik post-processor**
- **Part update pipeline** — geometry diff, ICP registration, operation audit
- **Stock simulation** — Z-buffer material removal
- **CAD integrations** — Onshape API, FreeCAD CLI bridge, watch folder
