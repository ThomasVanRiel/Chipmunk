# Phase 4: CLI 2.5D Milling — SVG Color Workflow

**Goal**: Profile, pocket, facing driven by SVG stroke colors + YAML job file. One `chipmunk mill` command generates all NC programs for a job.

---

## SVG Color Workflow

Paths in the SVG are selected by **stroke color**. Circles → drill points; closed paths → profile or pocket. The YAML maps each hex color to a full operation config.

Inkscape workflow:
1. Draw part in Inkscape
2. Assign stroke colors per operation type (e.g. red = profile outside, orange = pocket)
3. Save as SVG
4. `chipmunk job.yaml --output part.H`

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
geometry: part.svg
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
# Full job → single NC file
chipmunk job.yaml --output part.H
# or: chipmunk job.yaml > part.H

# Dry run: show what would be generated without writing files
chipmunk job.yaml --dry-run
# prints:
#   #00ff00: 4 circles → drill T1 (Center Drill 3mm)
#   #0000ff: 4 circles → drill T2 (Drill 6mm)
#   #ff0000: 1 closed path → profile outside T3 (6mm End Mill)
#   #ff8800: 2 closed paths → pocket T3 (6mm End Mill)

# Single-color override (roughing + finishing from same YAML)
chipmunk job.yaml --color "#ff0000" --allowance 0.2 --output rough.H
chipmunk job.yaml --color "#ff0000" --allowance 0.0 --output finish.H

# Override geometry file (revised part, same operations)
chipmunk job.yaml --geometry part_v2.svg --output part_v2.H

# Override post-processor without editing the YAML
chipmunk job.yaml --postprocessor haas --output part.nc

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

### `chipmunk mill` subcommand
- [ ] `src/cli/mill.rs` — handler:
  1. Load `JobParams` from positional YAML arg; resolve `geometry:` path (relative to YAML)
  2. Apply CLI overrides: `--geometry`, `--postprocessor`, `--color`, `--allowance`
  3. Parse geometry file → `Vec<ColorGroup>`
  4. For each `OperationConfig`: find matching `ColorGroup` by color
  5. Warn on geometry colors not in job YAML; error on job colors with no geometry matches
  6. Run operation → `Vec<ToolpathSegment>`
  7. Compile NCBlocks
  8. Run Lua post-processor → NC string
  9. Write to `--output <path>` or stdout
- [ ] `--dry-run` flag — print color groups + matched operations, exit 0
- [ ] `--color <hex>` flag — process only one color group, combine with per-field overrides
- [ ] `--postprocessor <name>` flag — override `postprocessor:` from YAML
- [ ] `--geometry <file>` flag — override `geometry:` from YAML
- [ ] `--allowance <f>` flag — override `allowance:` on all matching operations
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

### Haas post-processor (Lua) — milling support
- [ ] `postprocessors/haas.lua` — extend with G1/G2/G3, G41/G42, G90 guard, M7/M8

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
- [ ] Integration test: `chipmunk mill fixture_job.yaml --dry-run` → expected group report
- [ ] Add fixtures: `tests/fixtures/job.svg`, `tests/fixtures/job.yaml` (with `geometry: job.svg`)

---

## Deliverable

```bash
chipmunk job.yaml --output part.H
```

One YAML (referencing a color-coded SVG) → NC output. Full 2.5D workflow: center drill + through drill + profile + pocket, no frontend required.
