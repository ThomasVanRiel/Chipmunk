# Ideas

## inbox

* Drilling cycles: Insert acknowledge line in manual drilling cycle so the operator knows to enable single block mode
* Usage.md: Tools should also mention their origin when checking a file.
* Usage.md: manual drilling should not use optional stop. Spindle speed will be activated before movements.
* Usage.md: Color mismatch should not be a warning but a hard error. The user asks the impossible.

---

## Decided

**CLI output to stdout**
Omitting `--output` prints to stdout. Also support `--output -` explicitly (Unix convention). Already specced in phase-2.

**Post-processor error returns**
The Lua post-processor can return `nil, "error message"` (idiomatic Lua) to signal an error — e.g. overtravel, unsupported cycle. Chipmunk prints the message to stderr and exits with code 1. Error content is free-form string; no structured error types needed.

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
