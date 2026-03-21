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

A `drill` subcommand to test the full pipeline without a frontend. Uses `clap` for argument parsing.

```bash
# From DXF — extract circle centers automatically
camproject drill holes.dxf \
  --tool-number 1 --tool-name "Center Drill" --diameter 3 \
  --spindle-speed 2000 \
  --clearance 5 \
  --postprocessor heidenhain \
  --output center_drill.H

# Explicit coordinates (no DXF needed)
camproject drill \
  --at 25,15 --at 75,15 --at 75,65 \
  --tool-number 1 --tool-name "Drill" --diameter 6 \
  --spindle-speed 1800 \
  --postprocessor heidenhain

# Print to stdout (no --output flag)
camproject drill holes.dxf --postprocessor heidenhain

# List available post-processors
camproject postprocessors
```

### CLI tasks
- [ ] Add `clap` to `Cargo.toml`
- [ ] `src/main.rs` — top-level subcommand dispatch: `serve` (existing), `drill`, `postprocessors`
- [ ] `drill` subcommand args: `[dxf_file]`, `--at X,Y` (repeatable), `--tool-number`, `--tool-name`, `--diameter`, `--spindle-speed`, `--feed`, `--clearance`, `--postprocessor`, `--output`
- [ ] `drill` handler:
  - If `dxf_file` given: import → extract circle centers
  - If `--at` given: use those coordinates directly
  - Build minimal `Tool`, `Setup`, `Operation` in memory (no project file, no DB)
  - Generate drill toolpath, compile NC IR, run Lua post-processor
  - Write to `--output` or print to stdout
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
camproject drill holes.dxf --postprocessor heidenhain --output DRILL.H
```
Transfer `DRILL.H` to Heidenhain TNC. Activate single block mode. Press cycle start — machine rapids to first hole, stops. Operator drills with quill or hand drill, presses cycle start, machine advances to next position. No `STOP` blocks, no spindle commands — single block mode gives the operator full pacing control.
