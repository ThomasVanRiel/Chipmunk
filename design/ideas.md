# Ideas

## inbox

* Toolpath calculations will be multithreaded.
* Pocket definitions in yaml? Some canned cycles support this.
* Postamble and preamble is not part of the IR. Program start and program end should not be IR blocks.
* CAD -> CAM, maar ook CAM -> CAD
* Maybe using subcommands is better for consistency, with a shortcut if a file is the first command?
  * `chipmunk run <args>` or `chipmunk <args>`
  * `chipmunk postprocessors`: but how can we emit pp capabilities (e.g. canned cycles)?
  * `chipmunk check`
  * `chipmunk serve`
  * `chipmunk tools`: list available tools with their origin and properties
* Add option to add M0 or M1 between operations
* It should be clearer from docs that the intent of this project is to be a CAM kernel, not only a CLI tool.

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

---

## Deferred

**`chipmunk wizard`**
Interactive CLI subcommand that prompts the user step by step (operation type, coordinates, tool, post-processor, output). For quick jobs without a drawing or YAML. Deferred — implement after core workflow is solid.

**Hooks to send NC to the machine**
Removed — piping stdout to a transfer tool is sufficient and more flexible.
