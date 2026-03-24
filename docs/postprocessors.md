# Writing a Post-Processor

A post-processor is a Lua script that converts Chipmunk's intermediate representation (IR) into machine-specific NC code. You don't need to know Rust or touch the core — just Lua and your controller's programming manual.

The built-in Heidenhain post-processor in `postprocessors/heidenhain.lua` is a working example. Read it alongside this document.

---

## Quick Start

1. Create a `.lua` file in `~/.config/chipmunk/postprocessors/` (Linux/macOS) or `%APPDATA%\chipmunk\postprocessors\` (Windows).
2. The filename (minus `.lua`) becomes the post-processor ID used in YAML job files.
3. Return a module table with `name`, `file_extension`, and a `generate()` function.

Minimal example:

```lua
local M = {}

M.name = "My Controller"
M.file_extension = ".nc"

function M.generate(blocks, context)
    local out = {}
    out[#out+1] = "(Program: " .. context.name .. ")"

    for _, block in ipairs(blocks) do
        if block.type == "rapid" then
            out[#out+1] = string.format("G0 X%.3f Y%.3f Z%.3f",
                block.x or 0, block.y or 0, block.z or 0)
        elseif block.type == "tool_change" then
            out[#out+1] = string.format("T%d M6", block.tool_number)
        elseif block.type == "comment" then
            out[#out+1] = "(" .. block.comment .. ")"
        elseif block.type == "stop" then
            out[#out+1] = "M0"
        elseif block.type == "spindle_on" then
            -- handle or skip
        elseif block.type == "spindle_off" then
            out[#out+1] = "M5"
        end
    end

    out[#out+1] = "M30"
    return table.concat(out, "\n")
end

return M
```

Run it:

```bash
chipmunk job.yaml --postprocessor mycontroller
```

If a user post-processor has the same ID as a built-in, it overrides the built-in.

---

## Module Interface

Your Lua file must return a table. The required and optional fields:

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Human-readable name (e.g. `"Haas VF-2"`) |
| `file_extension` | string | yes | Output file extension including dot (e.g. `".nc"`, `".h"`) |
| `generate(blocks, context)` | function | yes | Takes IR blocks and context, returns NC code string. On error: return `nil, "message"`. |
| `supported_cycles` | table | no | List of canned cycle type strings this PP handles. Omit for no cycle support. |
| `optional_skip_strategy` | string | no | `"block_delete"` (default) or `"jump"`. Controls how optional operations are skipped. |
| `format_block(block)` | function | no | Convenience — format a single block as one line. Not called by Chipmunk; use it from your own `generate()` if you want. |

### generate(blocks, context)

This is the only function Chipmunk calls. It receives the full IR block list and a context table, and must return the complete NC program as a single string.

**Return values:**
- Success: return the NC string. A trailing newline is added automatically if missing.
- Error: return `nil, "descriptive error message"`. Chipmunk prints the message to stderr and exits with code 1. Use this for machine-specific validation (overtravel, unsupported block type, etc.).

### supported_cycles

```lua
M.supported_cycles = { "drill", "peck_drill", "bore", "tap" }
```

When declared, the NC compiler emits `cycle_define` / `cycle_call` / `cycle_off` blocks for matching drill strategies instead of explicit moves. If omitted or empty, your PP only receives basic motion blocks — no cycle handling needed.

Cycle types: `"drill"`, `"peck_drill"`, `"spot_drill"`, `"bore"`, `"tap"`, `"chip_break"`.

---

## IR Block Reference

Every block passed to `generate()` is a Lua table with a `type` field (string) and type-specific parameters. All parameter values are numbers or strings — no nested tables.

Coordinates (`x`, `y`, `z`) are `nil` when unchanged from the previous block (modal optimization). Always check for `nil` before formatting.

### Currently Implemented

These block types are emitted by the current codebase:

| Type | Parameters | Description |
|---|---|---|
| `tool_change` | `tool_number` (number or nil), `spindle_speed` (number) | Tool change. Heidenhain merges spindle speed into `TOOL CALL`. |
| `comment` | `comment` (string) | Operator comment. Note: the param is `comment`, not `text`. |
| `stop` | *(none)* | Mandatory program stop (M0). |
| `spindle_on` | `direction` (`"cw"` or `"ccw"`) | Start spindle. No speed — speed is on `tool_change`. |
| `spindle_off` | *(none)* | Stop spindle. |
| `rapid` | `x` (number/nil), `y` (number/nil), `z` (number/nil) | Rapid positioning move. Any axis can be nil (not moving on that axis). |

### Planned (Not Yet Implemented)

These block types are defined in the design but not yet emitted. They will arrive as more operation types are implemented. Your PP can handle them now or return `nil` to skip.

| Type | Parameters | Description |
|---|---|---|
| `linear` | `x`, `y`, `z`, `f` (feed rate, nil if modal) | Linear interpolation (G1) |
| `arc_cw` | `x`, `y`, `z`, `i`, `j`, `f` | Clockwise arc (G2). `i`/`j` are center offsets from start. |
| `arc_ccw` | `x`, `y`, `z`, `i`, `j`, `f` | Counter-clockwise arc (G3) |
| `coolant_on` | `mode` (`"flood"`, `"mist"`, `"through_tool"`) | Coolant on |
| `coolant_off` | *(none)* | Coolant off |
| `dwell` | `p` (seconds) | Dwell / pause (G4) |
| `set_units` | `units` (`"mm"` or `"inch"`) | Set units mode (G20/G21) |
| `set_work_offset` | `offset` (e.g. `"G54"`) | Set work coordinate offset |
| `set_plane` | *(tbd)* | Set working plane (G17/G18/G19) |
| `set_mode` | *(tbd)* | Absolute/incremental (G90/G91) |
| `set_feed_mode` | *(tbd)* | Feed per minute/rev (G94/G95) |
| `comp_left` | `d` (offset register number) | Cutter compensation left (G41) |
| `comp_right` | `d` (offset register number) | Cutter compensation right (G42) |
| `comp_off` | *(none)* | Cancel cutter compensation (G40) |
| `optional_stop` | *(none)* | Optional stop (M1) |
| `program_end` | *(none)* | Program end (M30/M2) |
| `cycle_define` | `cycle_type`, `z`, `r`, `f`, `q` (nil if N/A), `pitch` (nil if N/A) | Define a canned cycle. Only sent if your PP declares the cycle type in `supported_cycles`. |
| `cycle_call` | `x`, `y` | Execute the active cycle at this position |
| `cycle_off` | *(none)* | Cancel the active cycle |
| `optional_skip_start` | `skip_level` (1-9), `label`, `operation_name` | Begin skippable section |
| `optional_skip_end` | `skip_level` (1-9), `label` | End skippable section |

### Context Table

The second argument to `generate()`:

| Field | Type | Description |
|---|---|---|
| `name` | string | Program name (from YAML `name:` field, or filename if omitted) |
| `units` | string | `"MM"` or `"INCH"` |

Planned additions: `num_tools`, `date`, `estimated_time_min`.

---

## base.lua Helpers

A `base.lua` file is loaded into the Lua VM before your post-processor. Its functions are available as globals — no `require` needed.

### Current API

```lua
Fmt(n, decimals)
```

Format a number to the given decimal places. Returns a string.

```lua
Fmt(10.5, 3)    -- "10.500"
Fmt(-0.1, 2)    -- "-0.10"
```

That's the only helper currently available. The Heidenhain PP defines its own `hh_coord()` and `format_coords()` as module functions — use these as examples of patterns you may need.

---

## Patterns

### Handling Nil Coordinates

The compiler omits coordinates that haven't changed. Always guard:

```lua
local function coords(block)
    local s = ""
    if block.x ~= nil then s = s .. string.format(" X%.3f", block.x) end
    if block.y ~= nil then s = s .. string.format(" Y%.3f", block.y) end
    if block.z ~= nil then s = s .. string.format(" Z%.3f", block.z) end
    return s
end
```

### Signed Coordinates (Heidenhain)

Heidenhain requires explicit `+` signs on positive values:

```lua
function M.hh_coord(axis, value)
    local sign = value >= 0 and "+" or ""
    return axis .. sign .. Fmt(value, 3)
end
```

### Line Numbering

Heidenhain requires sequential line numbers on every line. Track state:

```lua
-- In generate():
local n = 0
for _, block in ipairs(blocks) do
    local line = M.format_block(block)
    if line then
        out[#out+1] = n .. " " .. line
        n = n + 1
    end
end
```

Note: the current Heidenhain PP uses `#lines` as the line counter, which counts from the number of accumulated output lines.

### Merging Blocks

Some controllers merge what the IR represents as separate blocks. Heidenhain merges spindle start with the next motion line (`L X... FMAX M3`), and cutter compensation with the move (`L X... RL`).

Approaches:
- **Buffer and emit later**: When you see `spindle_on`, store the M-code and append it to the next motion line.
- **Return empty string**: The current Heidenhain PP returns `""` for `spindle_on`/`spindle_off` as a placeholder. This works but means the spindle command is silently dropped — to be fixed as the PP matures.

### Canned Cycles (Future)

When your PP declares `supported_cycles`, the compiler sends `cycle_define` instead of explicit drill moves. Map cycle types to your controller's native syntax:

```lua
-- G-code example
if block.type == "cycle_define" then
    if block.cycle_type == "peck_drill" then
        return string.format("G83 Z%.3f R%.3f Q%.3f F%.0f",
            block.z, block.r, block.q, block.f)
    end
elseif block.type == "cycle_call" then
    return string.format("X%.3f Y%.3f", block.x, block.y)
elseif block.type == "cycle_off" then
    return "G80"
end
```

If your controller doesn't support cycles, don't declare `supported_cycles` — the compiler falls back to explicit rapid/linear moves automatically.

### Optional Operations (Future)

For block delete (default strategy), prefix lines within `optional_skip_start`/`optional_skip_end` with `/`:

```lua
local in_skip = false

if block.type == "optional_skip_start" then
    in_skip = true
elseif block.type == "optional_skip_end" then
    in_skip = false
end

-- When formatting any other block:
local prefix = in_skip and "/ " or ""
out[#out+1] = prefix .. formatted_line
```

For the jump strategy (Heidenhain, Sinumerik), emit controller-specific conditional jumps around the section.

---

## Unknown Block Types

Your PP will encounter new block types as Chipmunk evolves. Decide on a strategy:

- **Error on unknown** (current Heidenhain approach): return `nil` from `format_block()`, which triggers the `nil, "unimplemented block: ..."` error return. Safe — forces you to handle every block explicitly.
- **Skip unknown**: return `nil` from `format_block()` and treat it as a no-op in `generate()`. More permissive but may silently drop important instructions.

The Heidenhain PP currently errors on unknown blocks. This is the recommended approach — it's better to fail loudly than to produce NC code that silently omits a spindle stop.

---

## Testing

Run your PP against a test YAML file and inspect the output:

```bash
# See the NC output
chipmunk tests/fixtures/drill.yaml --postprocessor mycontroller

# Diff against expected output
chipmunk tests/fixtures/drill.yaml --postprocessor mycontroller | diff expected.nc -

# Compare with the built-in Heidenhain output
chipmunk tests/fixtures/drill.yaml
```

The test fixtures in `tests/fixtures/` are minimal YAML files you can use as input. See `tests/fixtures/drill.yaml` for a manual drill example with three points.

---

## Discovery

Post-processors are found in two locations, checked in order:

1. `postprocessors/` directory next to the chipmunk binary (built-in)
2. `~/.config/chipmunk/postprocessors/` (user, Linux/macOS) or `%APPDATA%\chipmunk\postprocessors\` (Windows)

The filename minus `.lua` is the ID. `base.lua` is reserved (loaded as helpers, excluded from the PP list). If a user file has the same name as a built-in, the user file wins.

List available post-processors:

```bash
chipmunk postprocessors
```
