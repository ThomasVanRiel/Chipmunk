# Phase 4: CLI 2.5D Milling — SVG Color Workflow

**Goal**: Profile, pocket, facing driven by SVG stroke colors + YAML job file. One `camproject mill` command generates all NC programs for a job.

---

## SVG Color Workflow

Paths in the SVG are selected by **stroke color**. Circles → drill points; closed paths → profile or pocket. The YAML maps each hex color to a full operation config.

Inkscape workflow:
1. Draw part in Inkscape
2. Assign stroke colors per operation type (e.g. red = profile outside, orange = pocket)
3. Save as SVG
4. `camproject mill part.svg --params job.yaml --output-dir ./nc/`

Suggested color convention (user-defined in YAML, not hardcoded):

| Color | Suggested use |
|-------|--------------|
| `#00ff00` | Center drill / spot drill |
| `#0000ff` | Through drill |
| `#ff0000` | Profile outside |
| `#ff00ff` | Profile inside |
| `#ff8800` | Pocket |
| `#00ffff` | Facing |

---

## YAML Job File

```yaml
# job.yaml — one file per part
postprocessor: heidenhain
clearance: 5.0

operations:
  - color: "#00ff00"
    type: drill
    tool_number: 1
    tool_name: "Center Drill 3mm"
    tool_diameter: 3.0
    spindle_speed: 2000

  - color: "#0000ff"
    type: drill
    tool_number: 2
    tool_name: "Drill 6mm"
    tool_diameter: 6.0
    depth: 20.0
    strategy: peck
    peck_depth: 4.0
    spindle_speed: 1500

  - color: "#ff0000"
    type: profile
    side: outside
    tool_number: 3
    tool_name: "6mm End Mill"
    tool_diameter: 6.0
    depth: 8.0
    stepdown: 2.0
    allowance: 0.0
    lead_in: true
    compensation: cam
    spindle_speed: 8000
    feed_rate: 800

  - color: "#ff8800"
    type: pocket
    tool_number: 3
    tool_name: "6mm End Mill"
    tool_diameter: 6.0
    depth: 6.0
    stepdown: 2.0
    stepover: 2.4
    entry: helix
    helix_radius: 3.0
    spindle_speed: 8000
    feed_rate: 800
```

All per-operation fields are optional where defaults are sensible. Top-level fields (`postprocessor`, `clearance`) apply to the whole job.

---

## CLI

```bash
# Full job → one NC file per tool
camproject mill part.svg --params job.yaml --output-dir ./nc/
# → nc/T1_CENTER_DRILL_3MM.H
# → nc/T2_DRILL_6MM.H
# → nc/T3_6MM_END_MILL.H   (profile + pocket share a tool → one file)

# Dry run: show what would be generated without writing files
camproject mill part.svg --params job.yaml --dry-run
# prints:
#   #00ff00: 4 circles → drill T1 (Center Drill 3mm)
#   #0000ff: 4 circles → drill T2 (Drill 6mm)
#   #ff0000: 1 closed path → profile outside T3 (6mm End Mill)
#   #ff8800: 2 closed paths → pocket T3 (6mm End Mill)

# Single-color override (roughing + finishing from same SVG)
camproject mill part.svg --params job.yaml --only-color "#ff0000" --allowance 0.2 --output rough.H
camproject mill part.svg --params job.yaml --only-color "#ff0000" --allowance 0.0 --output finish.H

# Unknown colors in SVG are listed as warnings but don't fail the job
```

---

## Backend Tasks

### YAML job file parsing
- [ ] Add `serde_yaml` to `Cargo.toml`
- [ ] `src/cli/job.rs` — `JobParams` struct: `postprocessor`, `clearance`, `operations: Vec<OperationConfig>`
- [ ] `OperationConfig` — `color: String`, `type: OperationType`, and all per-type params as `Option<T>`
- [ ] Validate: error if required fields missing (type, tool_number, tool_diameter); warn on unknown colors in SVG

### Color-keyed geometry grouping
- [ ] `io/svg_reader.rs` — extend to return `Vec<ColorGroup>` where each group has `color: String` and `Vec<SvgEntity>` (circle, closed path, open path)
- [ ] `io/dxf_reader.rs` — same for DXF entity colors (ACI → hex, or RGB)
- [ ] `SvgEntity` — enum: `Circle { center: Point2, radius: f64 }`, `ClosedPath(geo::Polygon)`, `OpenPath(geo::LineString)`
- [ ] Normalize colors to lowercase 6-digit hex `#rrggbb`; handle SVG named colors (e.g. `red` → `#ff0000`) and `rgb(r,g,b)` syntax

### `camproject mill` subcommand
- [ ] `src/cli/mill.rs` — handler:
  1. Parse SVG → `Vec<ColorGroup>`
  2. Load `JobParams` from YAML
  3. For each `OperationConfig`: find matching `ColorGroup` by color
  4. Warn on SVG colors not in job YAML; error on job colors with no SVG matches
  5. Run operation → `Vec<ToolpathSegment>`
  6. Compile NCBlocks
  7. Group NCBlocks by tool number
  8. Run Lua post-processor per tool group → NC string
  9. Write to `--output-dir/<Tn_name>.<ext>` or stdout if single tool + no dir
- [ ] `--dry-run` flag — print color groups + matched operations, exit 0
- [ ] `--only-color <hex>` flag — process only one color group, combine with per-field overrides
- [ ] Tool ordering: within a program, operations run in the order they appear in `operations:` array

### Polygon offset
- [ ] `toolpath/offset.rs` — `offset_polygon(poly: &Polygon, distance: f64) -> MultiPolygon`
- [ ] `iterative_offset(poly, tool_diameter, stepover) -> Vec<MultiPolygon>`

### Depth strategy
- [ ] `toolpath/depth_strategy.rs` — `compute_depth_passes(depth, stepdown) -> Vec<f64>`

### Toolpath generators
- [ ] `toolpath/facing.rs` — `FacingOperation`: zigzag raster, configurable stepover and angle
- [ ] `toolpath/profile.rs` — `ProfileOperation`: inside/outside/on, CAM mode (offset), controller mode (exact + comp blocks), tangent lead-in, depth passes
- [ ] `toolpath/pocket.rs` — `PocketOperation`: contour-parallel/zigzag, helix/plunge/ramp entry, depth passes

### NC IR extensions
- [ ] `nc/ir.rs` — add: `LinearMove { x, y, z, feed }`, `ArcMove { x, y, z, i, j, clockwise, feed }`, `SetFeedRate`, `CoolantOn`, `CoolantOff`, `CutterCompLeft`, `CutterCompRight`, `CutterCompOff`

### Heidenhain milling (Lua)
- [ ] `postprocessors/heidenhain.lua` — extend:
  - `LinearMove` → `L X+n Y+n Z+n F+n`
  - `ArcMove` → `CC X+n Y+n` + `C X+n Y+n DR+` or `DR-`
  - `CutterCompLeft` → `RL`, `CutterCompRight` → `RR`, `CutterCompOff` → `R0`
  - `CoolantOn` → `M8`, `CoolantOff` → `M9`

### Other post-processors (Lua) — milling support
- [ ] `postprocessors/linuxcnc.lua` — G1/G2/G3, G41/G42, M7/M8
- [ ] `postprocessors/grbl.lua` — G1/G2/G3, no comp, no coolant
- [ ] `postprocessors/marlin.lua` — G1/G2/G3, M3/M5
- [ ] `postprocessors/fanuc.lua` — G1/G2/G3, G41/G42, G90 guard

---

## Tests

- [ ] `tests/test_color_grouping.rs` — SVG with 3 stroke colors → 3 groups with correct entity types and counts
- [ ] SVG color normalization: `red`, `#FF0000`, `rgb(255,0,0)` all normalize to `#ff0000`
- [ ] `tests/test_offset.rs` — 100×100 square offset inward 3mm → 94×94
- [ ] `tests/test_depth_strategy.rs` — depth=10, stepdown=2 → passes at [2,4,6,8,10]
- [ ] `tests/test_facing.rs` — polygon, 5mm stepover → expected row count
- [ ] `tests/test_profile.rs` — outside profile CAM mode → offset contour; controller mode → exact + comp blocks
- [ ] `tests/test_pocket.rs` — pocket contour-parallel → offset loops; helix entry
- [ ] Golden files: `tests/fixtures/nc/heidenhain_profile.H`, `heidenhain_pocket.H`, `heidenhain_facing.H`
- [ ] Integration test: `camproject mill fixture.svg --params fixture_job.yaml --dry-run` → expected group report
- [ ] Add fixtures: `tests/fixtures/job.svg`, `tests/fixtures/job.yaml`

---

## Deliverable

```bash
camproject mill part.svg --params job.yaml --output-dir ./nc/
```

One SVG (drawn in Inkscape with color-coded paths) + one YAML → complete set of NC files for the job. Each tool gets its own `.H` file. Full 2.5D workflow: center drill + through drill + profile + pocket, no frontend required.
