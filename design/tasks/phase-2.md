# Phase 2: Manual Drill + Heidenhain Export

**Goal**: First complete end-to-end workflow, validated via CLI before any frontend work.

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

---

## Backend

### Tool system (minimal)
- [ ] `core/tool.rs` — `Tool` struct: `id`, `tool_number`, `name`, `machine`, `diameter`, `ToolType` (Drill, EndMill, etc.)

### Setup / WCS (minimal)
- [ ] `core/setup.rs` — `Setup` struct: `id`, `name`, `wcs_origin` (XYZ), `clearance_height`

### Operation + drill toolpath
- [ ] `core/operation.rs` — `Operation` struct: `id`, `setup_id`, `tool_id`, `name`, `params: OperationParams`
- [ ] `DrillParams` — `points: Vec<Point2>`, `spindle_speed`, `feed_rate`
- [ ] `toolpath/registry.rs` — `OperationType` trait: `type_id`, `validate`, `generate`, `compile_nc`
- [ ] `toolpath/drill.rs` — `DrillOperation` impl:
  - `generate()`: scan geometry for circles → return `Vec<ToolpathSegment::DrillPoint(x, y)>`
  - `compile_nc()`: returns `None` in manual mode (generic compiler handles it)

### NC IR (minimal)
- [ ] `nc/ir.rs` — `NCBlock`, `BlockType` enum:
  - `RapidMove { x, y, z }`
  - `LinearMove { x, y, z, feed }`
  - `ProgramStop` (M0 — unconditional)
  - `OptionalStop` (M1 — operator-controlled)
  - `SpindleOn { speed, clockwise }`
  - `SpindleOff`
  - `ToolChange { tool_number, name }`
  - `ProgramEnd`

### NC compiler (minimal)
- [ ] `nc/compiler.rs` — `compile_program()`:
  - Header blocks (program name, units)
  - Tool change block
  - Rapid to clearance height (no spindle on for manual strategy)
  - For each drill point: `RapidMove(x, y, clearance)` → `RapidMove(x, y)` (XY only)
  - Rapid to clearance, program end

### Lua bridge + post-processor registry
- [ ] `nc/bridge.rs`:
  - `PostProcessorCapabilities` struct (read from Lua module fields before compilation)
  - `get_capabilities(module)` — reads `supported_cycles`, `optional_skip_strategy`
  - `generate_nc(blocks, context, pp_name) -> String` — fresh `Lua` instance, load base.lua + pp module, call `M.generate(blocks, context)`
- [ ] `nc/postprocessors/mod.rs` — `BUILTIN_POSTPROCESSORS: &[(&str, &str)]` = `[(name, lua_source)]`
- [ ] `nc/postprocessors/builtin.rs` — `include_str!` for each `.lua` file

### Lua post-processors
- [ ] `postprocessors/base.lua`:
  - `M.fmt(n, dec)` — format number to fixed decimal places
  - `M.feed(f)` — format feed rate
  - `M.hh_coord(v)` — Heidenhain coordinate with explicit sign (`+`/`-`) and 3 decimal places
  - `M.hh_num(v)` — same but no sign stripping (for Q-parameters)
- [ ] `postprocessors/heidenhain.lua` (manual drill mode):
  - `M.name = "Heidenhain TNC"`
  - `M.file_extension = "H"`
  - `M.supported_cycles = {}` (empty for now)
  - `M.optional_skip_strategy = "m1"`
  - `M.generate(blocks, ctx)` — `BEGIN PGM`, `BLK FORM`, `TOOL CALL n Z S0`, `L Z+clearance FMAX`, then for each `RapidMove`: `L X+n Y+n FMAX`, then `L Z+clearance FMAX`, `END PGM` — no `M3`/`M5`, no `STOP`; operator uses single block mode

---

## CLI

All input via YAML. Two forms: geometry file (circles extracted as drill points) or explicit `points:` list.

```yaml
# drill.yaml — from geometry file
geometry: holes.dxf
postprocessor: heidenhain
clearance: 5.0

operations:
  - color: "#0000ff"
    type: drill
    strategy: manual
    tool_number: 1
    tool_name: "Center Drill"
    tool_diameter: 3.0
    spindle_speed: 2000
```

```yaml
# drill.yaml — explicit coordinates (no geometry file)
postprocessor: heidenhain
clearance: 5.0

operations:
  - type: drill
    strategy: manual
    tool_number: 1
    tool_name: "Drill"
    tool_diameter: 6.0
    spindle_speed: 1800
    points:
      - [25, 15]
      - [75, 15]
      - [75, 65]
```

```bash
chipmunk drill.yaml --output DRILL.H
# or: chipmunk drill.yaml > DRILL.H

chipmunk postprocessors
```

### CLI tasks
- [ ] Add `clap` to `Cargo.toml`
- [ ] `src/bin/chipmunk.rs` — positional YAML argument; `postprocessors` subcommand
- [ ] YAML handler: load `JobParams`, resolve `geometry:` if present, build `Tool`/`Setup`/`Operation` in memory, generate toolpath, compile NC IR, run Lua post-processor, write to `--output` or stdout
- [ ] Override flags: `--geometry`, `--postprocessor`, `--output`
- [ ] `postprocessors` subcommand — print table of `name | file_extension` for all registered post-processors
- [ ] Error messages: unknown post-processor, no points found, file not found — all to stderr with exit code 1

---

## Tests

- [ ] `tests/test_drill.rs` — DXF with 3 circles → 3 `DrillPoint` toolpath segments at correct XY
- [ ] `tests/test_nc_compiler.rs` — 3 drill points, manual strategy → expected NCBlock sequence (ToolChange, RapidMove×4, ProgramEnd) — no SpindleOn, no Stop blocks
- [ ] `tests/test_postprocessors.rs` — NCBlocks → Heidenhain output matches golden file
- [ ] Add fixture: `tests/fixtures/holes.dxf` (3 circles at known positions)
- [ ] Add golden file: `tests/fixtures/nc/heidenhain_manual_drill.H`

---

## Deliverable

Test the pipeline on real hardware using only the CLI:
```bash
chipmunk drill.yaml --output DRILL.H
```
Transfer `DRILL.H` to Heidenhain TNC. Activate single block mode. Press cycle start — machine rapids to first hole, stops. Operator drills with quill or hand drill, presses cycle start, machine advances to next position. No `STOP` blocks, no spindle commands — single block mode gives the operator full pacing control.
