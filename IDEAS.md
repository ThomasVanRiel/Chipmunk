# Ideas

## inbox

* STEP NC support + bidirectional CAD↔CAM data flow. STEP-NC (ISO 14649) describes what to make rather than how to move — controllers optimize toolpaths, results can feed back to CAD. Massive scope expansion. Few controllers support it natively (Siemens SINUMERIK partial). GD&T interpretation alone is a project.
* Implementing all Heidenhain conversational language features will result in extremely complex postprocessor bindings and IR. Implement a subset of the specification will be sufficient. Not all canned cycles and coordinate transformations are needed. How can we define the scope of the postprocessors?
* Should we add postprocessor parameters?

---

## Decided

**Tool origin in `chipmunk check`**
`chipmunk check job.yaml` shows each tool with its resolution level: user library, project, setup, or inline. Purely informational — no warnings about shadowing.

**Manual drilling: acknowledge line before cycle**
Before spindle on, emit a comment line (e.g. `; ENABLE SINGLE BLOCK MODE FOR MANUAL DRILLING`) followed by M0. The operator reads the comment, enables single block mode, and presses cycle start. No corresponding message at end of cycle.

**`--plot` flag for toolpath SVG output**
`--plot <path>` generates an SVG of toolpaths alongside normal NC generation (e.g. `chipmunk job.yaml --output part.H --plot toolpaths.svg`). Reflects the same operations as the NC output — respects `--tool` and `--color` filters. SVG contains separate layers (`<g>` groups) for original geometry, stock outline, and toolpaths, so they can be toggled in an SVG viewer. Color coded by operation. Within each operation color, rapids are dashed lines, feeds are solid.

**Drill point patterns**
`points:` accepts explicit `[x, y]` coordinates alongside pattern definitions: `circle_pattern` (center, radius, count, optional start_angle default 0), `line_pattern` (start, end, count or spacing — providing both is a hard error), `rect_pattern` (corner, spacing, count as `[columns, rows]`, optional angle default 0). Patterns are preserved through the IR — not flattened early — so post-processors can emit native pattern support (e.g. Heidenhain `PATTERN DEF`). Post-processors without native pattern support receive pre-expanded operations from Rust — pattern expansion is computation, not string formatting, so it stays out of Lua. Phase 2.

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

**CLI subcommands**
Adopt subcommands: `chipmunk run`, `chipmunk postprocessors`, `chipmunk check`, `chipmunk serve`, `chipmunk tools`. Implicit `run` when first arg is a `.yaml`/`.yml` file (shortcut: `chipmunk job.yaml`). `chipmunk postprocessors [name]` can show a specific PP's capabilities (supported cycles, skip strategy).

**M0/M1 between operations**
Per-setup YAML field `stop_between_operations`: `"mandatory"` (M0), `"optional"` (M1), or `"none"` (default). Compiler emits `stop` or `optional_stop` IR block between operations within a setup.

**Chipmunk is a CAM kernel**
Docs should frame Chipmunk as a CAM kernel with a CLI-first interface — not just a CLI tool. The architecture (pure computational library with thin CLI/API adapters) supports multiple frontends: CLI, REST API, future GUI, integrations.

**Positions and patterns on all operations**
Any operation can take `positions` and/or `patterns` fields (same syntax as drill points: explicit `[x, y]` coordinates, `circle_pattern`, `line_pattern`, `rect_pattern`). The operation executes at each position as an offset from WCS. If none are provided, the operation executes at WCS origin. Patterns are preserved through the IR so post-processors can emit native support (e.g. Heidenhain `PATTERN DEF` + `CYCL CALL PAT`). Post-processors without native pattern support receive pre-expanded operations from Rust.

**QR and datamatrix patterns**
`qr_pattern` and `datamatrix_pattern` are pattern types alongside `circle_pattern`, `line_pattern`, and `rect_pattern`. They generate a grid of positions from a content string and cell size. Same preservation principle: patterns stay in the IR so post-processors with native support can emit native instructions (e.g. Heidenhain data matrix engraving cycles); the compiler expands in Rust for post-processors without support.

**Tolerances in operation definitions**
Any operation can declare `tolerance` and `tolerance_strategy`. Two input formats (structurally distinct — string vs list):

* ISO fit (string): `tolerance: "H7"` — kernel resolves from nominal dimension in geometry. Case is validated: `H7` = hole tolerance, `h7` = shaft tolerance. Wrong case for the feature type is a hard error. Geometry must provide a nominal dimension or hard error.
* Explicit deviations (list): `tolerance: [-0.0, +0.1]` — deviations from nominal dimension in geometry. Requires known nominal or hard error.

`tolerance_strategy`: `wide` (target loose end), `middle` (center of band), `narrow` (target tight end).

> Heidenhain also provides tolerances in the CYCLE208 Bore Milling.

In **CAM mode**: the kernel shifts the target dimension and computes the offset toolpath accordingly. In **controller mode** (G41/G42, RL/RR): the programmed contour is shifted to the tolerance target and the post-processor emits a comment with the tolerance info so the operator knows what was applied. The operator retains full control via wear offsets. No tolerance declared = nominal geometry, no shift — operator manages everything via wear offsets.

**Post-processor capability declarations: cycles and patterns**
Post-processors declare supported capabilities in two separate categories: **cycles** (e.g. drilling cycles, pocket cycles) and **patterns** (e.g. circle, line, rect, qr, datamatrix). The compiler checks both when deciding whether to preserve IR constructs for native emission or pre-expand them in Rust.

**Lenient input, strict on missing data**
The kernel accepts multiple valid representations for the same concept — each must be structurally unambiguous so the kernel pattern-matches on input shape, never guesses. Examples: a coordinate can be an `[x, y, z]` array or a color reference to geometry (structurally distinct); colors can be `"#FF0000"`, `"red"`, or `"rgb(255,0,0)"` (different formats, same value); feeds can be absolute RPM or cutting parameters (different field sets, never mixed). If two input formats could be confused, that's a design bug. Anything not explicitly stated is a hard error — never warn, never infer.

**Modal state and incremental moves in the IR**
The IR supports modal controller state (coordinate mode, feed mode, working plane, etc.) via operation-level settings blocks emitted at the start of each operation. Individual move blocks can override the operation default (e.g., a single incremental retract within an otherwise absolute operation). The post-processor tracks current modal state and emits switches (G90/G91, `I...` prefix, G94/G95, etc.) only when state changes. This keeps the IR declarative while preserving semantic intent — an incremental retract ("up 2mm from here") is distinct from an absolute move to a Z height.

---

## Deferred

**`chipmunk wizard`**
Interactive CLI subcommand that prompts the user step by step (operation type, coordinates, tool, post-processor, output). For quick jobs without a drawing or YAML. Deferred — implement after core workflow is solid.

**Hooks to send NC to the machine**
Removed — piping stdout to a transfer tool is sufficient and more flexible.

**Heidenhain contour-based canned cycles**
How to pass contour geometry to Heidenhain cycles like `CYCL DEF 25x` (contour pocket, contour milling). These expect contours defined as labeled subprograms (`LBL`), structurally different from G-code. Deferred until contour milling operations are implemented.
