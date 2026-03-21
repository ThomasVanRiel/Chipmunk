# Phase 3: Automatic Drill Cycles + Per-Program Export

**Goal**: Native canned cycles (Heidenhain CYCL DEF, G-code G81/G83). Per-program export for no-tool-changer workflow. Full drilling use case complete.

## Context: No-Tool-Changer Workflow

Without an ATC, the operator sets Z=0 at the **tool tip** before each program run — no tool length measurement or compensation needed. Each tool gets its own program file:

1. Load center drill → touch off Z at tip → run `CENTER_DRILL.H`
2. Load Ø6 drill → touch off Z at tip → run `DRILL_6.H`
3. Load Ø10 drill → touch off Z at tip → run `DRILL_10.H`

This means:
- **No `TOOL CALL` length compensation** in the NC output (no G43, no Heidenhain tool length call)
- **Clearance height is relative to Z=0 (tool tip)** — a clearance of +5mm means 5mm above where the tip touched off
- **One program per tool** is the natural export unit
- Each program is self-contained (spindle on, moves, spindle off)

---

## Backend

### Full DrillParams
- [ ] Extend `DrillParams`: `peck_depth: Option<f64>`, `chip_break_distance: Option<f64>`, `dwell: Option<f64>`, `retract_plane: f64`, `use_canned_cycle: bool`
- [ ] `DrillStrategy` enum: `Manual`, `Simple`, `Peck`, `ChipBreak`, `Bore`, `Tap { pitch: f64 }`
  - `Manual`: rapid to XY + `Stop` (M0); no Z motion, operator drills by hand. Already implemented in Phase 2 — just needs to be a named variant rather than implicit behaviour.
- [ ] Drill toolpath generator: produce explicit Z moves as fallback (`RapidMove` to clearance → `LinearMove` to depth → `RapidMove` back)

### `compile_nc` hook on `DrillOperation`
- [ ] Implement `compile_nc(op, caps) -> Option<Vec<NCBlock>>`:
  - Return `None` if `!op.params.use_canned_cycle` or `caps.supported_cycles` is empty
  - Return `None` if required cycle not in `caps.supported_cycles`
  - Emit `CycleDefine { cycle_type, params }` + `CycleCall { x, y }` × N + `CycleOff`
- [ ] `nc/ir.rs` additions: `CycleDefine { cycle_type: String, params: HashMap<String, f64> }`, `CycleCall { x, y }`, `CycleOff`
- [ ] NC compiler: call `op.compile_nc(caps)` first; fall through to `compile_toolpath_generic()` if `None`

### `PostProcessorCapabilities` on bridge
- [ ] `nc/bridge.rs` — `get_capabilities(lua_module) -> PostProcessorCapabilities`:
  - Read `M.supported_cycles` (array of strings)
  - Read `M.optional_skip_strategy` ("m1", "block_delete", or "none")
  - Read `M.tool_length_compensation` (bool) — false = Z-zero-at-tip mode, no G43/TNC length call
- [ ] Call `get_capabilities()` before compilation; pass `caps` to each `compile_nc()`

### Z-zero-at-tip mode
- [ ] `ProgramContext` — `tool_length_mode: ToolLengthMode` enum: `ZeroAtTip` (no compensation) or `MeasuredOffset` (emit G43/TNC equivalent)
- [ ] NC compiler: skip tool length compensation blocks when `ZeroAtTip`
- [ ] Heidenhain: `ZeroAtTip` → omit `TOOL CALL` length parameter; `MeasuredOffset` → include tool length in `TOOL CALL`

### Heidenhain canned cycles (Lua)
- [ ] `postprocessors/heidenhain.lua` — `format_cycl_def(block, state) -> string`:
  - CYCL DEF 200 (simple drill): Q200 retract, Q201 depth, Q206 feed, Q202 peck (=depth for simple), Q210 dwell, Q203 Z surface, Q204 2nd clearance
  - CYCL DEF 203 (peck drill): same Q-params + Q213 chip-break distance
  - CYCL DEF 207 (rigid tap): Q200, Q201, Q239 pitch, Q203, Q204
- [ ] Cycle call: `L X+n Y+n FMAX M99`
- [ ] Cycle off: emit nothing (TNC cancels cycle on next non-M99 move)
- [ ] Add `"cycl_def_200"`, `"cycl_def_203"`, `"cycl_def_207"` to `M.supported_cycles`

### G-code canned cycles (Lua)
- [ ] `postprocessors/haas.lua` — G81 (simple), G83 (peck, Q = peck depth), G84 (tap, F = pitch × RPM)
  - Emit `G80` after `CycleOff`
  - R-plane from `retract_plane` param
  - `G90` absolute mode guard

### Optional operations (M1 stop)
- [ ] `core/operation.rs` — `optional_skip_level: Option<u8>` (1–9) on `Operation`
- [ ] `nc/ir.rs` — `OptionalBlock { level: u8, blocks: Vec<NCBlock> }` wrapper
- [ ] NC compiler: wrap optional operation blocks in `OptionalBlock`
- [ ] `postprocessors/heidenhain.lua` — `OptionalBlock`: emit `M1` before block
- [ ] `postprocessors/haas.lua` — `OptionalBlock`: prefix lines with `/`

### Per-program export
- [ ] NC compiler: when compiling a single-tool program, omit `TOOL CALL` if `tool_length_mode == ZeroAtTip` (tool is already loaded and zeroed)
- [ ] `--output-dir` CLI flag: writes one NC file per tool, named `T<n>_<name>.<ext>`

---

## Tests

- [ ] `tests/test_drill_cycles.rs`:
  - Simple drill, `use_canned_cycle: true`, Heidenhain caps → `CycleDefine("cycl_def_200")` + `CycleCall`×N + `CycleOff`
  - Same op + caps without `cycl_def_200` → `None` → fallback explicit Z moves
  - Peck drill → `CycleDefine("cycl_def_203")`
  - Tap → `CycleDefine("cycl_def_207")`
- [ ] Golden files:
  - `tests/fixtures/nc/heidenhain_cycl200.H` — CYCL DEF 200, three holes, Z-zero-at-tip mode
  - `tests/fixtures/nc/heidenhain_cycl203.H` — CYCL DEF 203 peck
  - `tests/fixtures/nc/haas_g83.nc` — G83 peck
- [ ] Per-program export test: two-tool job → `--output-dir` → two NC files with correct operation subsets
- [ ] Optional operation test: `optional_skip_level: 1` → Heidenhain emits `M1`, Haas prefixes `/`

---

## Deliverable

Complete drilling workflow for no-tool-changer machine:
1. DXF with center punch marks + through holes
2. Create two drill operations: center drill (T1) and Ø6 drill (T2)
3. Set Z-zero mode to "Tool tip"
4. "Export by tool" → downloads ZIP with `T1_CENTER_DRILL.H` and `T2_DRILL_6.H`
5. Load center drill, touch off Z at tip, run `T1_CENTER_DRILL.H` — Heidenhain uses CYCL DEF 200
6. Load Ø6 drill, touch off Z at tip, run `T2_DRILL_6.H` — Heidenhain uses CYCL DEF 203 (peck)
