# Ideas

## inbox

* STEP NC support + bidirectional CAD↔CAM data flow. STEP-NC (ISO 14649) describes what to make rather than how to move — controllers optimize toolpaths, results can feed back to CAD. Massive scope expansion. Few controllers support it natively (Siemens SINUMERIK partial). GD&T interpretation alone is a project.
  * Maybe we can implement tolerances already in the operation definition. The tool path calculation can calculate the optimal path, maybe add flag `tolerance: [H7/-0.0+0.1]` and `tolerance_strategy: [wide/middle/narrow]`
* Add IR block goto clearance? Going to max Z is useful. Clearance moves contain only Z, which allows IR moves (rapid/linear) to always contain ALL used coordinates (Omitting multi axis is fine for now).
* Absolute vs relative moves? PP should handle this, as heidenhain uses I... while G-code uses G91 as a modal command.
* Path planning using heidenhain is rather complex. How to send contour to canned cycles?
* All operations can take positions and patterns which are offsets to WCS. If none are provided, they are exectured at WCS
* Design principle: User input can be lenient (e.g. color definitions in multiple formats, WCS definition can be color or coordinate in svg). Things that are not explicitely stated are errors. We never warn the user.
* Patterns: cirles, rectangles, datamatrix, QR, linear.
* Document IR blocks and possible operations and patterns better

---

## Decided

**CLI output to stdout**
Omitting `--output` prints to stdout. Also support `--output -` explicitly (Unix convention). Already specced in phase-2.

**Post-processor error returns**
The Lua post-processor can return `nil, "error message"` (idiomatic Lua) to signal an error — e.g. overtravel, unsupported cycle. Chipmunk prints the message to stderr and exits with code 1. Error content is free-form string; no structured error types needed.

**Tool origin in `chipmunk check`**
`chipmunk check job.yaml` shows each tool with its resolution level: user library, project, setup, or inline. Purely informational — no warnings about shadowing.

**Manual drilling: spindle on, acknowledge line, no optional stops**
Spindle is activated before movements (not S0). No optional stops (M01) between points — operator uses single block mode to step through. Combined with the acknowledge line below.

**Manual drilling: acknowledge line before cycle**
Before spindle on, emit a comment line (e.g. `; ENABLE SINGLE BLOCK MODE FOR MANUAL DRILLING`) followed by M0. The operator reads the comment, enables single block mode, and presses cycle start. No corresponding message at end of cycle.

**`--plot` flag for toolpath SVG output**
`--plot <path>` generates an SVG of toolpaths alongside normal NC generation (e.g. `chipmunk job.yaml --output part.H --plot toolpaths.svg`). Reflects the same operations as the NC output — respects `--tool` and `--color` filters. SVG contains separate layers (`<g>` groups) for original geometry, stock outline, and toolpaths, so they can be toggled in an SVG viewer. Color coded by operation. Within each operation color, rapids are dashed lines, feeds are solid.

**Drill point patterns**
`points:` accepts explicit `[x, y]` coordinates alongside pattern definitions: `circle_pattern` (center, radius, count, optional start_angle default 0), `line_pattern` (start, end, count or spacing — providing both is a hard error), `rect_pattern` (corner, spacing, count as `[columns, rows]`, optional angle default 0). Patterns are preserved through the IR — not flattened early — so post-processors can emit native pattern support (e.g. Heidenhain `PATTERN DEF`). `base.lua` provides `M.expand_patterns(blocks)` for post-processors without native support. Phase 2.

**Color mismatch is a hard error**
If an SVG contains paths with a stroke color that has no matching operation in the job YAML, Chipmunk exits with a hard error (exit code 1). The user asks the impossible — never silently skip geometry.

**Feeds and speeds: absolute or cutting parameters**
Two input modes for the same fields:

* Absolute: `spindle_speed: 8000` (RPM) + `feed_rate: 100` (mm/min)
* Cutting parameters: `cutting_speed: 80` (m/min) + `teeth: 4` + `chip_load: 0.02` (mm/tooth) — Chipmunk computes RPM and mm/min from tool diameter

Controllers that support constant surface speed natively (Sinumerik `G96`, Heidenhain `CSS`) can have the post-processor emit native cutting speed instead of fixed RPM, if declared in post-processor capabilities.

**Multithreaded toolpath calculations**
Toolpath calculations parallelized per operation via rayon. Operations within a setup are independent — natural fit for data parallelism, no design implications.

**Pocket definitions in YAML**
Simple pockets (rectangular, circular) can be defined parametrically in YAML instead of requiring SVG/DXF. Preserved through the IR so post-processors can emit native pocket cycles (e.g. Heidenhain `CYCL DEF 251/252`) instead of computed toolpaths. Same principle as drill pattern preservation.

**Preamble/postamble is not part of the IR**
Program start and end are machine-specific. The post-processor handles them in `generate()` — no `program_end` or `program_start` IR blocks. Already implemented this way.

**CLI subcommands**
Adopt subcommands: `chipmunk run`, `chipmunk postprocessors`, `chipmunk check`, `chipmunk serve`, `chipmunk tools`. Implicit `run` when first arg is a `.yaml`/`.yml` file (shortcut: `chipmunk job.yaml`). `chipmunk postprocessors [name]` can show a specific PP's capabilities (supported cycles, skip strategy).

**M0/M1 between operations**
Per-setup YAML field `stop_between_operations`: `"mandatory"` (M0), `"optional"` (M1), or `"none"` (default). Compiler emits `stop` or `optional_stop` IR block between operations within a setup.

**Chipmunk is a CAM kernel**
Docs should frame Chipmunk as a CAM kernel with a CLI-first interface — not just a CLI tool. The architecture (pure computational library with thin CLI/API adapters) supports multiple frontends: CLI, REST API, future GUI, integrations.

---

## Deferred

**`chipmunk wizard`**
Interactive CLI subcommand that prompts the user step by step (operation type, coordinates, tool, post-processor, output). For quick jobs without a drawing or YAML. Deferred — implement after core workflow is solid.

**Hooks to send NC to the machine**
Removed — piping stdout to a transfer tool is sufficient and more flexible.
