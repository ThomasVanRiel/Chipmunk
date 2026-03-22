# NC Code Generation & Post-Processor System

## Overview

NC code generation is a two-stage process:

1. **Compilation** (Rust): Toolpath segments → controller-neutral `NCBlock` intermediate representation
2. **Post-processing** (Lua): `NCBlock` list → machine-specific NC code string

This separation means toolpath generators never need to know about G-code dialects, and post-processors never need to know about machining strategies. The Rust/Lua boundary sits between these two stages, connected via `mlua`.

Post-processors are written in Lua rather than Python to keep the distribution small. The entire Lua 5.4 VM adds ~300KB to the binary. Built-in post-processors are embedded as Lua source strings at compile time (`include_str!()`). User post-processors are `.lua` files dropped into a config directory — no installation or compilation required.

## Intermediate Representation (IR)

The IR is defined in Rust (`src/nc/ir.rs`). When passed to Lua, each `NCBlock` becomes a Lua table.

### NCBlock (Rust)

The atomic unit of NC output. Each block represents one logical instruction.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockType {
    Comment,
    Rapid,               // G0
    Linear,              // G1
    ArcCw,               // G2
    ArcCcw,              // G3
    ToolChange,          // M6
    SpindleOn,           // M3/M4
    SpindleOff,          // M5
    CoolantOn,           // M8/M7
    CoolantOff,          // M9
    Dwell,               // G4
    Stop,                // M0 — mandatory program stop
    OptionalStop,        // M1 — optional stop (operator switch)
    ProgramEnd,          // M30/M2
    SetUnits,            // G20/G21
    SetWorkOffset,       // G54-G59
    SetPlane,            // G17/G18/G19
    SetMode,             // G90/G91 (absolute/incremental)
    SetFeedMode,         // G94/G95 (per-minute/per-rev)
    CompLeft,            // G41 — cutter compensation left
    CompRight,           // G42 — cutter compensation right
    CompOff,             // G40 — cancel cutter compensation

    // Canned cycles (future — see "Canned Cycles" section below)
    CycleDefine,         // Define a cycle with parameters
    CycleCall,           // Execute the defined cycle at a position
    CycleOff,            // Cancel active cycle

    // Optional operation skip (see "Optional Operations" section below)
    OptionalSkipStart,   // Start of skippable section
    OptionalSkipEnd,     // End of skippable section
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NCBlock {
    pub block_type: BlockType,
    pub params: HashMap<String, serde_json::Value>,
    // Params vary by type:
    // Rapid/Linear: x, y, z, f (feed)
    // ArcCw/Ccw: x, y, z, i, j, k, f
    // ToolChange: t (tool number), tool_name
    // SpindleOn: s (speed), direction ("cw"/"ccw")
    // Dwell: p (seconds)
    // Comment: text
    // SetUnits: units ("mm"/"inch")
    // SetWorkOffset: offset ("G54"/"G55"/...)
    // CompLeft/Right: d (offset register number)
    // OptionalSkipStart: skip_level (1-9), label, operation_name
    // OptionalSkipEnd: skip_level (1-9), label
    // CycleDefine: cycle_type, + cycle-specific params
    // CycleCall: x, y (position to execute cycle at)
}
```

### NCBlock (Lua representation)

The `mlua` bridge converts each `NCBlock` to a Lua table before calling the post-processor. The `type` field uses snake_case strings. All param keys are lowercase.

```lua
-- Examples of block tables passed to Lua:
{ type = "rapid",        x = 0.0,   y = 0.0,   z = 25.0 }
{ type = "linear",       x = 10.0,  y = 20.0,  z = -5.0, f = 800.0 }
{ type = "arc_cw",       x = 50.0,  y = 20.0,  z = -5.0, i = 0.0, j = 5.0, f = 800.0 }
{ type = "tool_change",  t = 1,     tool_name = "6mm end mill" }
{ type = "spindle_on",   s = 18000, direction = "cw" }
{ type = "spindle_off" }
{ type = "coolant_on",   mode = "flood" }   -- mode: "flood", "mist", "through_tool"
{ type = "coolant_off" }
{ type = "comment",      text = "Rough pocket" }
{ type = "set_units",    units = "mm" }
{ type = "set_work_offset", offset = "G54" }
{ type = "comp_left",    d = 1 }
{ type = "comp_right",   d = 1 }
{ type = "comp_off" }
{ type = "dwell",        p = 0.5 }
{ type = "program_end" }
{ type = "optional_skip_start", skip_level = 1, label = "SKIP1", operation_name = "Finish profile" }
{ type = "optional_skip_end",   skip_level = 1, label = "SKIP1" }
```

The context table:
```lua
{
    project_name = "My Part",
    units = "mm",            -- "mm" or "inch"
    num_tools = 3,
    date = "2026-03-21",
    estimated_time_min = 12.3,  -- nil if not computed
}
```

### Compiler (Rust)

The compiler transforms a list of `Toolpath` objects into a complete NC program as a `Vec<NCBlock>`:

```rust
pub fn compile_program(
    operations: &[Operation],
    project: &Project,
) -> Vec<NCBlock> {
    // Compile all operations into a complete NC program.
}
```

**Compilation order**:
1. Program header comment (project name, date)
2. Safety line: units, absolute mode, XY plane, feed-per-minute
3. For each setup (operations grouped by `setup_id`):
   a. Work offset from setup WCS (G54/G55/...)
   b. For each enabled operation in the setup:
      - Comment block (if `operation.comment` is set — operator note in the NC file)
      - Tool change
      - Spindle on at specified RPM
      - Coolant on (if enabled)
      - Rapid to clearance height
      - Toolpath segments (rapid/linear/arc)
      - Rapid to clearance height
      - Spindle off (if last operation for this tool)
   c. Full retraction between setups (spindle off, coolant off, retract to safe Z)
4. Return to home
5. Program end (M30)

**Optimization**: The compiler tracks modal state and omits redundant values. Consecutive linear moves at the same feed rate omit `f` from the second block — the post-processor then omits `F` from that line.

---

## Post-Processor System

### Lua Module Interface

A post-processor is a Lua file that returns a module table. The minimum required fields are `name`, `file_extension`, and `generate`. Everything else is optional.

```lua
-- postprocessors/linuxcnc.lua
local base = require("base")   -- shared helpers (see below)
local M = {}

M.name           = "LinuxCNC"
M.file_extension = ".ngc"

-- Optional: declare which canned cycle types this post-processor supports.
-- Omit or return empty table for no cycle support (cycles expanded to explicit moves).
M.supported_cycles = { "drill", "peck_drill", "spot_drill", "bore", "tap" }

-- Optional: how to implement optional operation skipping.
-- "block_delete" (default) or "jump"
M.optional_skip_strategy = "block_delete"

-- Required: generate complete NC code from the block list.
-- blocks: array of block tables (see IR section above)
-- context: program context table
-- Returns: NC code as a single string on success.
-- On error: return nil, "descriptive error message"
-- Chipmunk prints the message to stderr and exits with code 1.
-- Use this for machine-specific validation (e.g. overtravel, unsupported cycle).
-- Content is free-form — no structured error types.
function M.generate(blocks, context)
    local out = {}

    -- Preamble
    out[#out+1] = "%"
    out[#out+1] = string.format("(%s)", context.project_name)
    out[#out+1] = string.format("(Generated: %s)", context.date)
    out[#out+1] = "G90 G94 G17"
    out[#out+1] = context.units == "mm" and "G21" or "G20"

    -- Blocks
    local n = 10
    for _, block in ipairs(blocks) do
        local line = M.format_block(block)
        if line then
            out[#out+1] = string.format("N%04d %s", n, line)
            n = n + 10
        end
    end

    -- Postamble
    out[#out+1] = "%"
    return table.concat(out, "\n") .. "\n"
end

-- Optional: format a single block as a line of NC code.
-- Return nil to skip the block (e.g., unsupported block types).
-- Used by the default generate() if you want block-by-block formatting
-- without rewriting the whole loop.
function M.format_block(block)
    if block.type == "rapid" then
        return string.format("G0%s", base.coords(block))
    elseif block.type == "linear" then
        return string.format("G1%s%s", base.coords(block), base.feed(block))
    elseif block.type == "arc_cw" then
        return string.format("G2%s%s", base.arc_coords(block), base.feed(block))
    elseif block.type == "arc_ccw" then
        return string.format("G3%s%s", base.arc_coords(block), base.feed(block))
    elseif block.type == "tool_change" then
        return string.format("T%d M6", block.t)
    elseif block.type == "spindle_on" then
        local dir = block.direction == "ccw" and "M4" or "M3"
        return string.format("S%.0f %s", block.s, dir)
    elseif block.type == "spindle_off" then
        return "M5"
    elseif block.type == "coolant_on" then
        return block.mode == "mist" and "M7" or "M8"
    elseif block.type == "coolant_off" then
        return "M9"
    elseif block.type == "dwell" then
        return string.format("G4 P%.3f", block.p)
    elseif block.type == "comment" then
        return string.format("(%s)", block.text)
    elseif block.type == "set_work_offset" then
        return block.offset
    elseif block.type == "comp_left" then
        return string.format("G41 D%02d", block.d)
    elseif block.type == "comp_right" then
        return string.format("G42 D%02d", block.d)
    elseif block.type == "comp_off" then
        return "G40"
    elseif block.type == "program_end" then
        return "M30"
    elseif block.type == "optional_skip_start" then
        M._in_skip = true
        M._skip_prefix = block.skip_level > 1
            and string.format("/%d ", block.skip_level) or "/ "
        return string.format("(%s)", "Optional: " .. block.operation_name)
    elseif block.type == "optional_skip_end" then
        M._in_skip = false
        return nil
    end
    return nil  -- unknown block types are skipped
end

return M
```

### `base.lua` — Shared Helpers

A shared `base.lua` module is embedded alongside the built-in post-processors. It provides coordinate formatting, number formatting, and other utilities post-processors commonly need.

```lua
-- base.lua (embedded, available via require("base"))
local M = {}

function M.fmt(value, places)
    places = places or 3
    return string.format("%." .. places .. "f", value)
end

-- Format XYZ coordinates present in a block (omits absent axes)
function M.coords(block)
    local s = ""
    if block.x ~= nil then s = s .. string.format(" X%.3f", block.x) end
    if block.y ~= nil then s = s .. string.format(" Y%.3f", block.y) end
    if block.z ~= nil then s = s .. string.format(" Z%.3f", block.z) end
    return s
end

-- Format IJK arc center offsets
function M.arc_coords(block)
    local s = M.coords(block)
    if block.i ~= nil then s = s .. string.format(" I%.3f", block.i) end
    if block.j ~= nil then s = s .. string.format(" J%.3f", block.j) end
    return s
end

-- Format feed rate (omit if nil — modal, unchanged from previous block)
function M.feed(block)
    if block.f then return string.format(" F%.0f", block.f) end
    return ""
end

return M
```

### mlua Bridge (Rust)

The `nc/bridge.rs` module handles the Rust → Lua → Rust roundtrip:

```rust
// nc/bridge.rs
use mlua::prelude::*;

pub struct LuaBridge {
    lua: Lua,
}

impl LuaBridge {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        // Load base.lua helper module
        let base_src = include_str!("../postprocessors/base.lua");
        lua.load(base_src).set_name("base").exec()?;
        Ok(Self { lua })
    }

    pub fn list_postprocessors(&self) -> Vec<PostProcessorInfo> {
        // Return info for all discovered post-processors (built-in + user)
    }

    pub fn generate_nc_code(
        &self,
        blocks: &[NCBlock],
        context: &ProgramContext,
        postprocessor_id: &str,
    ) -> Result<String> {
        let script = self.load_postprocessor(postprocessor_id)?;
        let pp: LuaTable = self.lua.load(&script).eval()?;

        let blocks_table = self.blocks_to_lua(blocks)?;
        let context_table = self.context_to_lua(context)?;

        let generate: LuaFunction = pp.get("generate")?;
        let result: String = generate.call((blocks_table, context_table))?;
        Ok(result)
    }
}
```

A new `Lua` instance is created per request (not shared across threads) because Lua is single-threaded. The `Lua` instance creation is cheap — the expensive part is loading scripts, so built-in post-processor sources are pre-loaded once at server startup and cached as strings.

### Plugin Discovery

Post-processors are discovered in two places:

**Built-in** (embedded at compile time):
```rust
// nc/postprocessors/mod.rs
pub const BUILTIN_POSTPROCESSORS: &[(&str, &str)] = &[
    ("linuxcnc", include_str!("linuxcnc.lua")),
    ("grbl",     include_str!("grbl.lua")),
    ("marlin",   include_str!("marlin.lua")),
    ("fanuc",    include_str!("fanuc.lua")),
    ("sinumerik",include_str!("sinumerik.lua")),
    ("heidenhain",include_str!("heidenhain.lua")),
];
```

**User-defined** (scanned at startup from config directory):
```
~/.config/chipmunk/postprocessors/   (Linux/macOS)
%APPDATA%\chipmunk\postprocessors\   (Windows)
```

Any `.lua` file in this directory is loaded and registered. The filename (minus extension) becomes the post-processor ID. User post-processors with the same ID as a built-in override the built-in.

### Writing a Custom Post-Processor

Create a `.lua` file and place it in the user postprocessors directory:

```lua
-- ~/.config/chipmunk/postprocessors/haas.lua
local base = require("base")
local M = {}

M.name           = "Haas"
M.file_extension = ".nc"
M.supported_cycles = { "drill", "peck_drill", "bore", "tap" }

function M.generate(blocks, context)
    local out = {}
    out[#out+1] = string.format("%%")
    out[#out+1] = string.format("O0001 (%s)", context.project_name)

    local n = 10
    for _, block in ipairs(blocks) do
        local line = M.format_block(block)
        if line then
            out[#out+1] = string.format("N%d %s", n, line)
            n = n + 10
        end
    end

    out[#out+1] = "M30"
    out[#out+1] = "%"
    return table.concat(out, "\n") .. "\n"
end

function M.format_block(block)
    -- Haas is Fanuc-compatible with minor differences
    if block.type == "tool_change" then
        -- Haas tool change syntax
        return string.format("T%02d M6", block.t)
    end
    -- Delegate to built-in Fanuc for everything else
    -- (in practice, you'd copy-paste or require a shared base)
    return nil
end

return M
```

No installation required. Restart the server and the post-processor appears in the list.

### Built-in Post-Processors

Two post-processors are built in. Others can be added by placing a `.lua` file in the config directory.

#### Haas (`.nc`)

The Haas post-processor is the built-in G-code example and serves as a starting point for other G-code controllers (Fanuc, LinuxCNC, Grbl, Sinumerik, etc.).

Key characteristics:
- `O0001` program number header
- Line numbers: `N1`, `N2`, ...
- Trailing decimal points: `X10.`
- `T1 M6` tool change
- `G90` absolute mode guard in preamble
- `M30` with rewind for program end
- G81 (simple drill), G83 (peck, Q = peck depth), G84 (tap)
- `G80` after `CycleOff`
- Block delete: `/` prefix for optional blocks

#### Heidenhain TNC (`.h`)

Heidenhain uses **conversational programming** — a completely different syntax from G-code. The Heidenhain post-processor overrides `generate()` entirely rather than using `format_block()`.

**Key syntax differences**:

| Concept | G-code | Heidenhain |
|---------|--------|------------|
| Line numbering | `N10` (optional) | Mandatory, every line: `0`, `1`, `2`, ... |
| Rapid move | `G0 X10 Y20 Z5` | `L X+10 Y+20 Z+5 FMAX` |
| Linear feed | `G1 X10 Y20 F500` | `L X+10 Y+20 F500` |
| Arc CW | `G2 X10 Y20 I5 J0` | `CR X+10 Y+20 R+5 DR-` |
| Arc CCW | `G3 X10 Y20 I5 J0` | `CR X+10 Y+20 R+5 DR+` |
| Tool change | `T1 M6` | `TOOL CALL 1 Z S18000` |
| Spindle on | `S18000 M3` | (included in `TOOL CALL`) |
| Comp left | `G41 D1` | `L ... RL` (appended to move) |
| Comp right | `G42 D1` | `L ... RR` (appended to move) |
| Comp off | `G40` | `L ... R0` (appended to move) |
| Program end | `M30` | `END PGM name MM` |
| Stock def | (none) | `BLK FORM 0.1 Z X...` |
| Coordinates | `X10.000` | `X+10` (explicit sign always required) |

The Heidenhain post-processor maintains a line counter and handles the structural mapping in its `generate()` override. Cutter compensation (`RL`/`RR`/`R0`) is appended to move lines rather than emitted as separate blocks, so the generator looks ahead in the block list.

**Drilling on Heidenhain**:

The Heidenhain post-processor declares cycle support via `supported_cycles`. The Rust compiler sees this and emits `CycleDefine`/`CycleCall`/`CycleOff` blocks instead of explicit moves. The post-processor maps each cycle type to its `CYCL DEF` number and formats the Q-parameters:

```lua
M.supported_cycles = { "drill", "peck_drill", "spot_drill", "bore", "tap" }

-- Heidenhain coordinates always carry an explicit sign
local function hh_coord(v)
    return v >= 0 and string.format("+%.3f", v) or string.format("%.3f", v)
end

local function hh_num(v)
    return v >= 0 and string.format("+%-7.3f", v) or string.format("%-7.3f", v)
end

local function format_cycl_def(block, state)
    local lines = {}

    if block.cycle_type == "drill" then
        lines[#lines+1] = string.format("%d CYCL DEF 200 DRILLING ~", state.n)
        lines[#lines+1] = string.format("  Q200=%s ;SET-UP CLEARANCE",     hh_num(2.0))
        lines[#lines+1] = string.format("  Q201=%s ;DEPTH",                hh_num(block.z))
        lines[#lines+1] = string.format("  Q206=%s ;FEED RATE PLUNGING",   hh_num(block.f))
        lines[#lines+1] = string.format("  Q202=%s ;INFEED DEPTH",         hh_num(math.abs(block.z)))
        lines[#lines+1] = string.format("  Q210=%s ;DWELL TIME AT TOP",    hh_num(0))
        lines[#lines+1] = string.format("  Q203=%s ;SURFACE COORDINATE",   hh_num(0))
        lines[#lines+1] = string.format("  Q204=%s ;2ND SET-UP CLEARANCE", hh_num(10))
        lines[#lines+1] = string.format("  Q211=%s ;DWELL TIME AT BOTTOM", hh_num(0))

    elseif block.cycle_type == "peck_drill" then
        lines[#lines+1] = string.format("%d CYCL DEF 203 UNIVERSAL DRILLING ~", state.n)
        lines[#lines+1] = string.format("  Q200=%s ;SET-UP CLEARANCE",      hh_num(2.0))
        lines[#lines+1] = string.format("  Q201=%s ;DEPTH",                 hh_num(block.z))
        lines[#lines+1] = string.format("  Q206=%s ;FEED RATE PLUNGING",    hh_num(block.f))
        lines[#lines+1] = string.format("  Q202=%s ;PLUNGING DEPTH",        hh_num(block.q))
        lines[#lines+1] = string.format("  Q210=%s ;DWELL TIME AT TOP",     hh_num(0))
        lines[#lines+1] = string.format("  Q203=%s ;SURFACE COORDINATE",    hh_num(0))
        lines[#lines+1] = string.format("  Q204=%s ;2ND SET-UP CLEARANCE",  hh_num(10))
        lines[#lines+1] = string.format("  Q212=%s ;DECREMENT",             hh_num(0))
        lines[#lines+1] = string.format("  Q213=%s ;BREAKS",                hh_num(3))
        lines[#lines+1] = string.format("  Q205=%s ;MIN. PLUNGING DEPTH",   hh_num(0))
        lines[#lines+1] = string.format("  Q211=%s ;DWELL TIME AT BOTTOM",  hh_num(0))
        lines[#lines+1] = string.format("  Q208=%s ;FEED RATE RETRACTION",  hh_num(99999))
        lines[#lines+1] = string.format("  Q256=%s ;DIST CHIP BREAKING",    hh_num(0.2))

    elseif block.cycle_type == "tap" then
        lines[#lines+1] = string.format("%d CYCL DEF 207 RIGID TAPPING ~", state.n)
        lines[#lines+1] = string.format("  Q200=%s ;SET-UP CLEARANCE",     hh_num(2.0))
        lines[#lines+1] = string.format("  Q201=%s ;DEPTH",                hh_num(block.z))
        lines[#lines+1] = string.format("  Q239=%s ;PITCH",                hh_num(block.pitch))
        lines[#lines+1] = string.format("  Q203=%s ;SURFACE COORDINATE",   hh_num(0))
        lines[#lines+1] = string.format("  Q204=%s ;2ND SET-UP CLEARANCE", hh_num(10))
    end

    state.n = state.n + 1
    return table.concat(lines, "\n")
end

-- In M.generate(), cycle blocks are handled alongside regular motion blocks:
-- ...
if block.type == "cycle_define" then
    out[#out+1] = format_cycl_def(block, state)
elseif block.type == "cycle_call" then
    -- Position move with M99 — triggers the active cycle at this XY position
    out[#out+1] = string.format("%d L X%s Y%s FMAX M99",
        state.n, hh_coord(block.x), hh_coord(block.y))
    state.n = state.n + 1
elseif block.type == "cycle_off" then
    -- TNC cancels the cycle implicitly after the last M99 when a non-cycle
    -- move follows. Explicit cancel only needed when switching cycle types.
    -- Emit nothing.
end
```

Output for a peck drill at three points, depth 25mm, peck 5mm:

```
42 CYCL DEF 203 UNIVERSAL DRILLING ~
  Q200=+2.000  ;SET-UP CLEARANCE
  Q201=-25.000 ;DEPTH
  Q206=+200.000 ;FEED RATE PLUNGING
  Q202=+5.000  ;PLUNGING DEPTH
  Q210=+0.000  ;DWELL TIME AT TOP
  Q203=+0.000  ;SURFACE COORDINATE
  Q204=+10.000 ;2ND SET-UP CLEARANCE
  Q212=+0.000  ;DECREMENT
  Q213=+3.000  ;BREAKS
  Q205=+0.000  ;MIN. PLUNGING DEPTH
  Q211=+0.000  ;DWELL TIME AT BOTTOM
  Q208=+99999.000 ;FEED RATE RETRACTION
  Q256=+0.200  ;DIST CHIP BREAKING
43 L X+10.000 Y+10.000 FMAX M99
44 L X+30.000 Y+10.000 FMAX M99
45 L X+50.000 Y+10.000 FMAX M99
```

If the target post-processor declares no cycle support, `DrillOperation::compile_nc` returns `None`, the compiler falls back to `compile_toolpath_generic`, and the post-processor receives ordinary `Rapid`/`Linear` blocks — no cycle handling needed.

**Example output** (facing operation):
```
0  BEGIN PGM MYPART MM
1  BLK FORM 0.1 Z X-0 Y-0 Z-20
2  BLK FORM 0.2 X+100 Y+80 Z+0
3  TOOL CALL 1 Z S18000
4  M8
5  L X-5 Y-5 FMAX M3
6  L Z+5 FMAX
7  L Z-0.5 F300
8  L X+105 F800
...
53 END PGM MYPART MM
```

---

## Drill Strategies

The `DrillOperation` supports several strategies, selected via `strategy:` in the YAML job file. All strategies share the same geometry input (a list of XY positions) but produce different NC output.

| Strategy | YAML value | Description |
|---|---|---|
| Manual | `manual` | Comment + M0 acknowledge, spindle on, rapid to each XY at clearance. Operator enables single block mode after acknowledge, drills with quill or hand tool, presses cycle start to advance. No Z motion from the machine. |
| Simple | `simple` | Machine feeds down to full depth in one pass, retracts. No `CycleDefine` — explicit moves. |
| Peck | `peck` | Feeds in increments (`peck_depth`), retracts fully between pecks to clear chips. Emits `CycleDefine(peck_drill)` when post-processor supports it. |
| Chip break | `chip_break` | Feeds in increments, retracts by a small amount (not fully) to break chips without clearing them. |
| Bore | `bore` | Feeds down at boring feed rate, retracts at same rate (oriented). |
| Tap | `tap` | Synchronised feed: `feed = pitch × RPM`. |

### Manual Drill

The manual strategy produces an XY traversal program with no Z motion. The program starts with a comment and M0 telling the operator to enable single block mode, then activates the spindle and rapids to each position. The operator drills each hole by hand (quill or hand drill) and presses cycle start to advance to the next position.

Single block mode is preferable to inserting `STOP` (M0) blocks between positions because:
- The program stays clean — no stops cluttering the point list
- The operator can choose to advance multiple positions without stopping simply by disabling single block mode
- Single block mode is a standard operator skill

The NC output (Heidenhain):

```
TOOL CALL 1 Z S800
; ENABLE SINGLE BLOCK MODE FOR MANUAL DRILLING
M0
L Z+10.000 FMAX M3           ; raise to clearance, spindle on
L X+15.000 Y+20.000 FMAX    ; position over hole 1  ← stop here in single block
L X+45.000 Y+20.000 FMAX    ; position over hole 2  ← stop here in single block
L X+75.000 Y+20.000 FMAX    ; position over hole 3  ← stop here in single block
L Z+10.000 FMAX M5           ; retract, spindle off
```

In the IR, manual drill compiles to a `Comment` + `Stop` (acknowledge), then `SpindleOn`, `Rapid` blocks, and `SpindleOff`. No `CycleDefine`. Every post-processor handles this without any special cycle support.

```yaml
# YAML
strategy: manual
# depth, peck_depth, feed_rate are ignored; spindle_speed is used
```

---

## Canned Cycles

Canned cycles let the controller handle repetitive patterns internally. The IR supports them via `CycleDefine` / `CycleCall` / `CycleOff` blocks. Toolpath generators always produce explicit moves as fallback. Canned cycles are **preferred by default** (`use_canned_cycle: true`) — they are emitted when the post-processor declares support, and the explicit moves serve as the fallback when it does not. Set `use_canned_cycle: false` on an operation to force explicit moves regardless of post-processor capability.

Post-processors declare support via `supported_cycles`:

```lua
M.supported_cycles = { "drill", "peck_drill" }
-- If omitted or empty: all cycles fall back to explicit moves
```

### Cycle Types in the IR

```
CycleDefine params by cycle_type:

Drilling:
  cycle_type="drill":      z, r, f
  cycle_type="peck_drill": z, r, f, q (peck increment)
  cycle_type="spot_drill": z, r, f
  cycle_type="bore":       z, r, f, p (dwell)
  cycle_type="tap":        z, r, f, pitch

Milling (future):
  cycle_type="contour_pocket":  z, r, f, stepover, finish_allowance
  cycle_type="face_mill":       z, r, f, stepover, width, length
```

### G-code Cycle Format

```gcode
G83 Z-25.0 R2.0 Q5.0 F200   ; peck drill cycle definition
X10.0 Y10.0                   ; drill at position 1
X30.0 Y10.0                   ; drill at position 2
G80                            ; cancel cycle
```

| IR cycle_type | G-code |
|---|---|
| `drill` | `G81` |
| `peck_drill` | `G83` |
| `bore` | `G85`/`G86` |
| `tap` | `G84` |

### Heidenhain Cycle Format

```
CYCL DEF 200 DRILLING ~
  Q200=2    ;SET-UP CLEARANCE ~
  Q201=-25  ;DEPTH ~
  Q206=200  ;FEED RATE ~
  Q202=5    ;INFEED DEPTH
L X+10 Y+10 FMAX M99
L X+30 Y+10 FMAX M99
```

| IR cycle_type | Heidenhain |
|---|---|
| `drill` | `CYCL DEF 200` |
| `peck_drill` | `CYCL DEF 203` |
| `bore` | `CYCL DEF 201` |
| `tap` | `CYCL DEF 207` |
| `contour_pocket` | `CYCL DEF 251`/`252`/`273` |
| `face_mill` | `CYCL DEF 232` |

---

## Optional Operations

Operations can be marked `optional=true` so the operator can skip them at runtime without editing the NC program.

Each optional operation has a `skip_level` (1–9). Operations with the same skip level are skipped together.

| skip_level | Typical use |
|---|---|
| 1 | Finishing passes |
| 2 | Chamfer / deburring |
| 3 | Engraving / marking |
| 4–9 | User-defined |

### Strategy 1: Block Delete (`/`)

Every line within the optional section is prefixed with `/` (or `/2`, `/3`, etc.). The operator toggles the hardware switch on the controller panel.

```gcode
(Optional: Finish profile - Block delete level 1)
/ N0200 T2 M6
/ N0210 S24000 M3
/ N0220 G0 X0 Y0
/ N0230 G41 D02
...
/ N0300 G40
/ N0310 M5
```

Multi-level (Haas, Sinumerik): `/2 N0200 T2 M6`

The post-processor's `M.optional_skip_strategy` field controls which strategy is used. Block delete is the default.

### Strategy 2: Labels + Conditional Jumps

More flexible — the operator sets a variable to control which operations run.

**Heidenhain**:
```
FN 9: IF +Q1 EQU +0 GOTO LBL 10
50 TOOL CALL 2 Z S24000
...
LBL 10
```

**Sinumerik**:
```gcode
IF R1==0 GOTOF SKIP_FINISH
N200 T2 M6
...
SKIP_FINISH:
```

**LinuxCNC** (O-codes):
```gcode
O100 IF [#<_skip_finish> EQ 0]
  O100 GOTO 999
O100 ENDIF
N200 T2 M6
...
O999
```

### Safety

The compiler inserts a safe Z retract before `OptionalSkipStart`. After `OptionalSkipEnd`, it cancels compensation (`G40`) and rapids to safe Z — regardless of whether the section ran.

---

## Example Output

LinuxCNC output for a simple facing operation on 100×80mm stock:

```gcode
%
(My Part)
(Generated: 2026-03-21)
G90 G94 G17
G21
G54
N0010 T1 M6 (6mm end mill)
N0020 S18000 M3
N0030 M8
N0040 G0 X-5.000 Y-5.000
N0050 G0 Z5.000
N0060 G1 Z-0.500 F300.000
N0070 G1 X105.000 F800.000
N0080 G0 Z5.000
N0090 G0 Y-2.600
N0100 G1 Z-0.500 F300.000
N0110 G1 X-5.000 F800.000
...
N0500 G0 Z25.000
N0510 M5
N0520 M9
N0530 M30
%
```
