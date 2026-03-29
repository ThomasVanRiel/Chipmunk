# Writing a Post-Processor

A post-processor is a Lua script that converts Chipmunk's intermediate representation (IR) into machine-specific NC code. You don't need to know Rust or touch the core — just Lua and your controller's programming manual.

The built-in Heidenhain post-processor in `postprocessors/heidenhain.lua` is a working example. Read it alongside this document.

---

## Quick Start

1. Create a `.lua` file in one of the search directories (see [Discovery](#discovery) below).
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
                block.x, block.y, block.z)
        elseif block.type == "tool_change" then
            out[#out+1] = string.format("T%d M6", block.tool_number)
        elseif block.type == "comment" then
            out[#out+1] = "(" .. block.text .. ")"
        elseif block.type == "stop" then
            out[#out+1] = "M0"
        elseif block.type == "spindle_on" then
            -- handle or skip
        elseif block.type == "spindle_off" then
            out[#out+1] = "M5"
        end
        -- Unrecognised block types are silently ignored.
        -- Add elseif branches as you need more block types.
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
| `capabilities` | table | no | Declares optional PP features. Omit entirely if not needed — defaults to no cycle support. See [Capabilities](#capabilities). |

### generate(blocks, context)

This is the only function Chipmunk calls. It receives the full IR block list and a context table, and must return the complete NC program as a single string.

**Return values:**

- Success: return the NC string. A trailing newline is added automatically if missing.
- Error: return `nil, "descriptive error message"`. Chipmunk prints the message to stderr and exits with code 1. Use this for machine-specific validation (overtravel, unsupported block type, etc.).

### Capabilities

`M.capabilities` is a table that declares optional PP features. Currently only `cycles` is read by Chipmunk; `patterns` is planned.

```lua
M.capabilities = {
    -- Declare which canned cycle types this PP handles natively.
    -- Omit or leave empty for no cycle support.
    cycles = {
        drilling    = {},
        peck_drill  = {},
        bore        = {},
        tap         = {},
    },

    -- (Planned) Declare which drill point patterns this PP can emit natively.
    -- If a pattern type is not declared, Chipmunk expands it into individual points
    -- before passing them to generate(). If declared, the pattern block is passed
    -- through as-is so your PP can emit a native pattern cycle.
    patterns = {
        circular = {},
    },
}
```

For both `cycles` and `patterns`, each key is a type name and the value is an empty table (reserved for future per-type parameters). Omit the entire `capabilities` field if you need neither.

Cycle types: `"drilling"`, `"peck_drill"`, `"spot_drill"`, `"bore"`, `"tap"`, `"chip_break"`.

Pattern types (planned): `"circular"`, `"line"`, `"rect"`.

---

## IR Block Reference

Every block passed to `generate()` is a Lua table with a `type` field (string) and type-specific parameters. Parameter values are numbers, strings, or booleans — no nested tables.

### Design principles

- Always output as much information as possible in the IR blocks.
  - Plane coordinates are always provided fully; the postprocessor can optimise unchanged coordinates.
  - Feed rate is always emitted in linear moves; the postprocessor can optimise modal state.
- Program start and end is machine specific, the postprocessor handles it.

### Currently Implemented

These block types are emitted by the current codebase:

| Type | Parameters | Description |
|---|---|---|
| `operation_start` | `text` (string or nil) | Marks the beginning of an operation. Optional label. |
| `operation_end` | `text` (string or nil) | Marks the end of an operation. Optional label. |
| `tool_change` | `tool_number` (number or nil), `spindle_speed` (number) | Tool change. Heidenhain merges spindle speed into `TOOL CALL`. |
| `comment` | `text` (string) | Operator comment. |
| `stop` | *(none)* | Mandatory program stop (M0). |
| `spindle_on` | `direction` (`"cw"` or `"ccw"`) | Start spindle. No speed — speed is on `tool_change`. |
| `spindle_off` | *(none)* | Stop spindle. |
| `retract` | `height` (number) | Rapid retract to a specific clearance height. |
| `retract_full` | *(none)* | Rapid retract to machine home / maximum Z. |
| `rapid` | `x` (number), `y` (number), `z` (number) | Rapid positioning move. All three axes are always present. |
| `linear` | `x` (number), `y` (number), `z` (number), `feed` (number) | Linear interpolation move with feed rate. |

### Planned (Not Yet Implemented)

These block types are defined in the design but not yet emitted. They will arrive as more operation types are implemented. Your PP can handle them now or return `nil` to skip.

| Type | Parameters | Description |
|---|---|---|
| `cycle_drill` | `depth`, `surface_position`, `plunge_depth`, `feed`, `dwell_top`, `dwell_bottom`, `clearance`, `second_clearance`, `tip_trough` | Canned drill cycle. All fields are numbers except `tip_trough` (boolean). Defined in the IR but not yet emitted by any operation. |
| `arc_cw` | `x`, `y`, `z`, `i`, `j`, `feed` | Clockwise arc (G2). `i`/`j` are center offsets from start. |
| `arc_ccw` | `x`, `y`, `z`, `i`, `j`, `feed` | Counter-clockwise arc (G3). |
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
| `cycle_define` | `cycle_type`, `z`, `r`, `feed`, `q` (nil if N/A), `pitch` (nil if N/A) | Define a canned cycle. Only sent if your PP declares the cycle type in `capabilities.cycles`. |
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

Chipmunk ships a helper library (`base.lua`) compiled into the binary. It is available via `require` — nothing needs to exist on disk:

```lua
local base = require("base")
```

### Current API

```lua
base.Fmt(n, decimals)
```

Format a number to the given decimal places. Returns a string.

```lua
local Fmt = require("base").Fmt

Fmt(10.5, 3)    -- "10.500"
Fmt(-0.1, 2)    -- "-0.10"
```

That's the only helper currently available. The Heidenhain PP defines its own `hh_coord()` and `format_coords()` as module functions — use these as examples of patterns you may need.

---

## Patterns

### Handling Nil Coordinates

Currently all coordinates on `rapid` and `linear` blocks are always present (non-nil). When arc blocks are added, some axes may be omitted if unchanged. Guard defensively for future compatibility:

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
- **Emit a standalone line**: The current Heidenhain PP emits `"L M3"` for `spindle_on` and `"L M5"` for `spindle_off` as standalone lines. This is a placeholder — ideally these would be appended to the adjacent motion line (`L X+... FMAX M3`), which is the idiomatic Heidenhain form.

### Canned Cycles

**Current:** The compiler emits a single `cycle_drill` block per drill operation. It bundles the cycle definition and position list together. Map it to your controller's drill cycle:

```lua
if block.type == "cycle_drill" then
    -- G-code example
    return string.format("G81 Z%.3f R%.3f F%.0f",
        block.depth, block.surface_position, block.feed)
end
```

**Planned:** Split into `cycle_define` / `cycle_call` / `cycle_off` to allow one definition followed by individual position calls. When that lands, declare support to opt in:

```lua
M.capabilities = { cycles = { drilling = {}, peck_drill = {} } }

-- Then in generate():
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

If your controller doesn't support cycles, omit `capabilities` — the compiler will fall back to explicit rapid/linear moves.

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

The test fixtures in `tests/fixtures/` are minimal YAML files you can use as input. See `tests/fixtures/drill.yaml` for a three-point `quill` operation example (manual positioning, no power feed).

---

## Discovery

Post-processors are `.lua` files discovered from two directories, searched in order:

1. **`postprocessors/`** — relative to the current working directory. This is where built-in post-processors ship (the `postprocessors/` directory in the repo root).
2. **`<config_dir>/chipmunk/postprocessors/`** — the user config directory, resolved via the [`dirs`](https://docs.rs/dirs) crate (`dirs::config_dir()`):
   - Linux: `~/.config/chipmunk/postprocessors/`
   - macOS: `~/Library/Application Support/chipmunk/postprocessors/`
   - Windows: `C:\Users\<user>\AppData\Roaming\chipmunk\postprocessors\`

The first match wins — `find_postprocessor()` returns the first `.lua` file it finds with a matching name. This means a file in `postprocessors/` (CWD) takes priority over the user config directory.

The filename minus `.lua` is the post-processor ID. `base.lua` in the repository is the source for the compiled-in helper library (see [base.lua Helpers](#baselua-helpers)) and is excluded from the PP list.

When listing post-processors (`chipmunk postprocessors`), both directories are scanned, results are merged, sorted alphabetically, and deduplicated.

```bash
chipmunk postprocessors
```
