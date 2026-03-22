# Phase 1: Scaffolding + Manual Drill (Points in YAML)

**Goal**: First complete end-to-end workflow via CLI. Explicit XY points in YAML → manual drill toolpath → Heidenhain NC → run on machine.

---

## Tasks

### 1. Project scaffolding

- [ ] `Cargo.toml` with dependencies: clap, serde, serde_yaml, mlua, tracing, anyhow, thiserror
- [ ] `src/main.rs` — entry point, clap dispatch
- [ ] `src/lib.rs` — module declarations
- [ ] Module structure on disk: `src/core/`, `src/toolpath/`, `src/nc/`, `src/io/`
- [ ] `cargo build` succeeds with empty modules

### 2. Core types

- [ ] `core/units.rs` — `Units` enum (Mm, Inch)
- [ ] `core/tool.rs` — `Tool` struct (number, name, diameter, spindle_speed)
- [ ] `core/operation.rs` — `Operation`, `DrillParams`, `DrillStrategy::Manual`

### 3. YAML job parsing

- [ ] `io/job.rs` — parse YAML job file into `JobConfig`
- [ ] Support `points:` on drill operations (list of `[x, y]` coordinate pairs)
- [ ] No `geometry:` field required at this phase
- [ ] Missing required fields → hard error to stderr, exit 1
- [ ] Unknown fields → hard error (strict deserialization)

### 4. Manual drill toolpath

- [ ] `toolpath/drill.rs` — `DrillOperation`
- [ ] Input: `Vec<[f64; 2]>` points + clearance height
- [ ] Output: `Vec<ToolpathSegment>` — `Rapid` moves only (no Z feed motion for manual strategy)

### 5. NC IR

- [ ] `nc/ir.rs` — `NCBlock` enum with variants: `Comment`, `Stop`, `SpindleOn`, `SpindleOff`, `ToolChange`, `Rapid`, `ProgramEnd`
- [ ] Each block carries its data (coordinates, spindle speed, comment text, etc.)

### 6. NC compiler

- [ ] `nc/compiler.rs` — transforms toolpath + operation config into `Vec<NCBlock>`
- [ ] Manual drill sequence: tool change → comment + M0 acknowledge → spindle on → clearance rapid → rapid to each point → retract → spindle off → program end

### 7. mlua bridge

- [ ] `nc/bridge.rs` — create fresh Lua VM per NC generation call
- [ ] Load `base.lua`, then the selected post-processor module
- [ ] Convert `Vec<NCBlock>` to Lua table, call `M.generate(blocks)`
- [ ] Return NC string on success; on `nil, "error"` return → print to stderr, exit 1

### 8. Post-processor registry

- [ ] `nc/postprocessors/mod.rs` — `BUILTIN_POSTPROCESSORS` array
- [ ] Built-in post-processors embedded via `include_str!()`
- [ ] Lookup by name (e.g. `"heidenhain"`)

### 9. `postprocessors/base.lua`

- [ ] Shared Lua helpers loaded before every post-processor
- [ ] `M.fmt()` — number formatting
- [ ] `M.hh_coord()` — Heidenhain coordinate formatting with explicit sign (`X+25.000`, `Y-10.000`)

### 10. `postprocessors/heidenhain.lua`

- [ ] `M.generate(blocks)` — iterates NCBlocks, emits Heidenhain conversational format
- [ ] Header: `BEGIN PGM <name> MM`
- [ ] Tool call: `TOOL CALL <n> Z S<speed>`
- [ ] Comment: `; <text>`
- [ ] Stop: `M0`
- [ ] Spindle on: `M3` (appended to next motion block)
- [ ] Rapid: `L X+n Y+n FMAX`
- [ ] Spindle off: `M5` (appended to last motion block)
- [ ] Footer: `END PGM <name> MM`

### 11. CLI wiring

- [ ] `chipmunk <file.yaml>` — parse YAML, run pipeline, write NC to stdout
- [ ] `--output <path>` — write NC to file instead of stdout
- [ ] `--output -` — explicit stdout (same as omitting `--output`)
- [ ] `chipmunk postprocessors` — list available post-processors (name + file extension)
- [ ] No `--output` and no file → stdout
- [ ] Exit code 0 on success, 1 on any error

### 12. Tests

- [ ] `test_job_parsing.rs` — valid YAML parses correctly; missing fields → error; unknown fields → error
- [ ] `test_drill.rs` — manual drill points → correct toolpath segments
- [ ] `test_nc_compiler.rs` — toolpath → correct IR blocks (comment, stop, spindle, rapids)
- [ ] Golden file test: `tests/fixtures/nc/heidenhain_manual_drill.H` — full output comparison
- [ ] Test fixture: `tests/fixtures/drill.yaml`

---

## Milestones

### M1: Skeleton compiles

Tasks 1–2 complete. `cargo build` succeeds, `cargo test` runs (no tests yet). Module structure in place.

### M2: YAML → toolpath

Tasks 3–4 complete. YAML parsed into job config, drill points extracted, toolpath segments generated. Unit tests pass.

### M3: Toolpath → NC IR → Lua → string

Tasks 5–8 complete. Full pipeline from toolpath through IR to Lua post-processor produces NC output. Bridge tests pass.

### M4: Heidenhain output correct

Tasks 9–10 complete. `heidenhain.lua` produces correct manual drill output. Golden file test passes.

### M5: CLI end-to-end

Tasks 11–12 complete. `chipmunk drill.yaml --output DRILL.H` works. All tests pass.

---

## Deliverable

`chipmunk drill.yaml --output DRILL.H` → valid Heidenhain NC file → load on TNC → quill drill workflow works.

Input (`drill.yaml`):

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

Output (`DRILL.H`):

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
