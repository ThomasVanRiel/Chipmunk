# NC Code Generation & Post-Processor System

## Overview

NC code generation is a two-stage process:

1. **Compilation**: Toolpath segments → controller-neutral `NCBlock` intermediate representation
2. **Post-processing**: `NCBlock` list → machine-specific G-code string

This separation means toolpath generators never need to know about G-code dialects, and post-processors never need to know about machining strategies.

## Intermediate Representation (IR)

### NCBlock

The atomic unit of NC output. Each block represents one logical instruction.

```python
class BlockType(Enum):
    COMMENT = "comment"
    RAPID = "rapid"                 # G0
    LINEAR = "linear"               # G1
    ARC_CW = "arc_cw"              # G2
    ARC_CCW = "arc_ccw"            # G3
    TOOL_CHANGE = "tool_change"     # M6
    SPINDLE_ON = "spindle_on"       # M3/M4
    SPINDLE_OFF = "spindle_off"     # M5
    COOLANT_ON = "coolant_on"       # M8/M7
    COOLANT_OFF = "coolant_off"     # M9
    DWELL = "dwell"                 # G4
    STOP = "stop"                   # M0 — mandatory program stop
    OPTIONAL_STOP = "optional_stop" # M1 — optional stop (operator switch)
    PROGRAM_END = "program_end"     # M30/M2
    SET_UNITS = "set_units"         # G20/G21
    SET_WORK_OFFSET = "set_work_offset"  # G54-G59
    SET_PLANE = "set_plane"         # G17/G18/G19
    SET_MODE = "set_mode"           # G90/G91 (absolute/incremental)
    SET_FEED_MODE = "set_feed_mode" # G94/G95 (per-minute/per-rev)
    COMP_LEFT = "comp_left"         # G41 — cutter compensation left
    COMP_RIGHT = "comp_right"       # G42 — cutter compensation right
    COMP_OFF = "comp_off"           # G40 — cancel cutter compensation

    # Canned cycles (future — see "Canned Cycles" section below)
    CYCLE_DEFINE = "cycle_define"   # Define a cycle with parameters
    CYCLE_CALL = "cycle_call"       # Execute the defined cycle at a position
    CYCLE_OFF = "cycle_off"         # Cancel active cycle

    # Optional operation skip (see "Optional Operations" section below)
    OPTIONAL_SKIP_START = "optional_skip_start"  # Start of skippable section
    OPTIONAL_SKIP_END = "optional_skip_end"      # End of skippable section

@dataclass
class NCBlock:
    type: BlockType
    params: dict[str, float | str]
    # Params vary by type:
    # RAPID/LINEAR: X, Y, Z, F (feed)
    # ARC_CW/CCW: X, Y, Z, I, J, K, F
    # TOOL_CHANGE: T (tool number), tool_name
    # SPINDLE_ON: S (speed), direction ("cw"/"ccw")
    # DWELL: P (seconds)
    # COMMENT: text
    # SET_UNITS: units ("mm"/"inch")
    # SET_WORK_OFFSET: offset ("G54"/"G55"/...)
    # COMP_LEFT/RIGHT: D (offset register number)
    # OPTIONAL_SKIP_START: skip_level (1-9), label (str), operation_name (str)
    # OPTIONAL_SKIP_END: skip_level (1-9), label (str)
    # COMP_OFF: (no params)
    # CYCLE_DEFINE: cycle_type, + cycle-specific params (see Canned Cycles section)
    # CYCLE_CALL: X, Y (position to execute cycle at)
    # CYCLE_OFF: (no params)
```

### Compiler

The compiler transforms a list of `Toolpath` objects (from one or more operations) into a complete NC program as a list of `NCBlock`:

```python
def compile_program(
    operations: list[Operation],
    project: Project,
) -> list[NCBlock]:
    """
    Compile operations into a complete NC program.

    Inserts:
    - Program start (units, work offset, absolute mode)
    - For each operation: tool change, spindle on, coolant, toolpath, spindle off
    - Program end
    """
```

**Compilation order**:
1. Program header (comment with project name, date)
2. Safety line: units, absolute mode, XY plane, feed-per-minute
3. Work offset (G54 default)
4. For each enabled operation (in order):
   a. Tool change
   b. Spindle on at specified RPM
   c. Coolant on (if enabled)
   d. Rapid to safe Z height
   e. Toolpath segments (rapid/linear/arc)
   f. Rapid to safe Z height
   g. Spindle off (if last operation for this tool)
5. Return to home position
6. Program end (M30)

**Optimization**: The compiler tracks modal state (current position, current feed rate, current tool) and omits redundant values. For example, if two consecutive linear moves have the same feed rate, the second `NCBlock` omits the `F` parameter — the post-processor then omits `F` from that line.

## Post-Processor System

### PostProcessor ABC

```python
from abc import ABC, abstractmethod

class PostProcessor(ABC):
    """Base class for all post-processors."""

    @property
    @abstractmethod
    def name(self) -> str:
        """Human-readable name, e.g. 'LinuxCNC'."""
        ...

    @property
    @abstractmethod
    def file_extension(self) -> str:
        """Output file extension, e.g. '.ngc'."""
        ...

    @abstractmethod
    def format_block(self, block: NCBlock) -> str | None:
        """
        Format a single NCBlock as a line of NC code.
        Return None to skip the block (e.g., unsupported block types).
        """
        ...

    def preamble(self, context: ProgramContext) -> list[str]:
        """Lines to emit before the first block. Override for machine-specific headers."""
        return []

    def postamble(self, context: ProgramContext) -> list[str]:
        """Lines to emit after the last block. Override for machine-specific footers."""
        return []

    def format_number(self, value: float, decimal_places: int = 3) -> str:
        """Format a coordinate value. Override for dialect-specific formatting."""
        return f"{value:.{decimal_places}f}"

    def format_blocks(self, block: NCBlock, prev: NCBlock | None, next: NCBlock | None) -> list[str]:
        """
        Format a single NCBlock as one or more lines of NC code.
        Returns empty list to skip the block.

        The prev/next context allows post-processors that need to look ahead or behind
        (e.g., Heidenhain combining compensation with the next move).

        Default implementation delegates to format_block() for backwards compatibility.
        """
        line = self.format_block(block)
        return [line] if line is not None else []

    def generate(self, blocks: list[NCBlock], context: ProgramContext) -> str:
        """
        Generate complete NC code from a block list.

        Can be overridden entirely for non-G-code formats (e.g., Heidenhain
        conversational) where the block-by-block model doesn't map cleanly.
        """
        lines = self.preamble(context)
        for i, block in enumerate(blocks):
            prev_block = blocks[i - 1] if i > 0 else None
            next_block = blocks[i + 1] if i < len(blocks) - 1 else None
            lines.extend(self.format_blocks(block, prev_block, next_block))
        lines.extend(self.postamble(context))
        return "\n".join(lines) + "\n"

@dataclass
class ProgramContext:
    """Metadata passed to post-processors for header/footer generation."""
    project_name: str
    units: Units
    num_tools: int
    date: str
    estimated_time_min: float | None
```

### Built-in Post-Processors

#### LinuxCNC (`.ngc`)
- Line numbers: `N0010`, `N0020`, ...
- Trailing decimal points on integers: `X10.000`
- Percent signs wrapping program: `%` at start and end
- `M2` or `M30` for program end
- Full G-code set including canned cycles

#### Grbl (`.gcode`)
- No line numbers (saves bandwidth on serial)
- No percent signs
- `M2` for program end
- Limited G-code set (no canned cycles — drill cycles expanded to explicit moves)
- `$H` homing support as optional preamble

#### Generic Fanuc (`.nc`)
- Line numbers: `N1`, `N2`, ...
- `O0001` program number in header
- Trailing decimal points: `X10.`
- `M30` program end with rewind
- Standard Fanuc modal groups

#### Marlin (`.gcode`)
- No line numbers
- Designed for CNC-adapted 3D printers
- `M3`/`M5` for spindle (or laser)
- `G28` for homing
- Simpler preamble/postamble

#### Sinumerik (`.mpf`)
- Line numbers: `N10`, `N20`, ...
- `G0`/`G1`/`G2`/`G3` standard motion (same as Fanuc base)
- Cutter compensation: `G41`/`G42`/`G40` (standard)
- Block delete: `/1` through `/8`
- Conditional jumps: `IF condition GOTOF label` / `IF condition GOTOB label`
- Variables: `R1`-`R999` for parameters
- `M30` program end
- Subprogram support: `PROC` / `RET` (future)

#### Heidenhain TNC (`.h`)

Heidenhain uses **conversational programming**, a completely different syntax from G-code. This is not a G-code dialect — it's a separate language. The Heidenhain post-processor overrides `generate()` to handle the structural differences.

**Key syntax differences from G-code**:

| Concept | G-code | Heidenhain |
|---------|--------|------------|
| Line numbering | `N10` (optional) | Mandatory, every line: `0`, `1`, `2`, ... |
| Rapid move | `G0 X10 Y20 Z5` | `L X+10 Y+20 Z+5 FMAX` |
| Linear feed | `G1 X10 Y20 F500` | `L X+10 Y+20 F500` |
| Arc CW | `G2 X10 Y20 I5 J0` | `CR X+10 Y+20 R+5 DR-` (radius form) or `CC`/`C` (center form) |
| Arc CCW | `G3 X10 Y20 I5 J0` | `CR X+10 Y+20 R+5 DR+` or `CC`/`C` |
| Tool change | `T1 M6` | `TOOL CALL 1 Z S18000` |
| Spindle on | `S18000 M3` | (included in `TOOL CALL`) |
| Spindle off | `M5` | `M5` (same) |
| Comp left | `G41 D1` | `L ... RL` (appended to move) |
| Comp right | `G42 D1` | `L ... RR` (appended to move) |
| Comp off | `G40` | `L ... R0` (appended to move) |
| Coolant on | `M8` | `M8` (same) |
| Program end | `M30` | `M30` or `END PGM` |
| Stock def | (none) | `BLK FORM 0.1 Z X-0 Y-0 Z-20` / `BLK FORM 0.2 X+100 Y+80 Z+0` |
| Coordinates | `X10.000` | `X+10` or `X-10` (explicit sign always required) |

**Heidenhain-specific features**:
- `BLK FORM` in preamble defines stock for simulation (maps from `StockDefinition`)
- Coordinates always carry an explicit `+` or `-` sign
- Cutter compensation (`RL`/`RR`/`R0`) is appended to the move line, not a separate block — this is why `format_blocks()` receives `prev`/`next` context
- Arcs can use center-point form (`CC` to set center, then `C` to cut) or radius form (`CR`)
- Cycles use `CYCL DEF` / `CYCL CALL` syntax for drilling, pocketing, etc. (though we generate explicit moves rather than relying on controller cycles, to keep the CAM in control)

**Example output** (same facing operation as G-code example):

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
9  L Z+5 FMAX
10 L Y-2.6 FMAX
11 L Z-0.5 F300
12 L X-5 F800
...
50 L Z+25 FMAX
51 M5
52 M9
53 END PGM MYPART MM
```

**Implementation note**: The Heidenhain post-processor overrides `generate()` rather than relying solely on `format_block()`. It maintains state (current compensation mode, current tool, block numbering) and handles the structural mapping from NCBlocks to conversational syntax. This is acceptable — the PostProcessor ABC is designed to allow full `generate()` override for non-G-code formats.

### Plugin Discovery

Post-processors are discovered at runtime via Python entry points:

```toml
# In pyproject.toml of the main project or any plugin package:
[project.entry-points."camproject.postprocessors"]
linuxcnc = "camproject.postprocessors.linuxcnc:LinuxCNCPost"
grbl = "camproject.postprocessors.grbl:GrblPost"
```

```python
# nc/registry.py
from importlib.metadata import entry_points

def discover_postprocessors() -> dict[str, type[PostProcessor]]:
    """Discover all registered post-processors."""
    eps = entry_points(group="camproject.postprocessors")
    return {ep.name: ep.load() for ep in eps}
```

### Writing a Custom Post-Processor

A third-party package can provide a custom post-processor by:

1. Creating a class that extends `PostProcessor`
2. Registering it as an entry point in their `pyproject.toml`
3. Installing the package into the same Python environment

```python
# my_custom_post/haas.py
from camproject.nc.base import PostProcessor, NCBlock, ProgramContext

class HaasPost(PostProcessor):
    @property
    def name(self) -> str:
        return "Haas"

    @property
    def file_extension(self) -> str:
        return ".nc"

    def format_block(self, block: NCBlock) -> str | None:
        # HAAS-specific formatting
        ...
```

```toml
# my_custom_post/pyproject.toml
[project.entry-points."camproject.postprocessors"]
haas = "my_custom_post.haas:HaasPost"
```

## Canned Cycles (Future)

Canned cycles let the controller handle repetitive motion patterns internally rather than executing explicit move-by-move G-code. This results in shorter programs, and the controller can optimize the motion (e.g., faster retract in a drill cycle). Canned cycle support is **not in the initial implementation** but the IR and post-processor architecture are designed to accommodate it.

### Architecture

The approach is a **dual-path** design:

1. **Toolpath generators always produce explicit moves** (rapid/linear/arc segments). This is the universal fallback that works on every controller.
2. **The NC compiler optionally recognizes cycle-eligible patterns** and emits `CYCLE_DEFINE` / `CYCLE_CALL` blocks instead of explicit moves, when the target post-processor declares cycle support.
3. **Post-processors that don't support cycles** (e.g., Grbl) ignore `CYCLE_DEFINE`/`CYCLE_CALL` and fall back to the expanded moves. The compiler emits both forms: the cycle blocks and the equivalent explicit blocks wrapped in a `CYCLE_EXPANDED` group that the post-processor can choose between.

```python
class PostProcessor(ABC):
    # ...existing methods...

    @property
    def supported_cycles(self) -> set[str]:
        """
        Return the set of cycle types this post-processor supports natively.
        Default: empty (no cycle support — all cycles are expanded to explicit moves).
        Override to declare support, e.g.: {"drill", "peck_drill", "contour_pocket"}
        """
        return set()
```

### Cycle Types in the IR

```python
# CYCLE_DEFINE params by cycle_type:

# Drilling cycles
# cycle_type="drill":         Z (final depth), R (retract plane), F (feed)
# cycle_type="peck_drill":    Z, R, F, Q (peck increment)
# cycle_type="spot_drill":    Z, R, F
# cycle_type="bore":          Z, R, F, P (dwell at bottom)
# cycle_type="tap":           Z, R, F, pitch

# Milling cycles (more complex — controller-specific)
# cycle_type="contour_pocket":   Z, R, F, stepover, finish_allowance
# cycle_type="contour_profile":  Z, R, F, approach_type
# cycle_type="face_mill":        Z, R, F, stepover, width, length
```

### How Post-Processors Format Cycles

#### G-code (Fanuc/LinuxCNC/Haas)

Drilling cycles use modal G-codes with position calls:

```gcode
G83 Z-25.0 R2.0 Q5.0 F200   (peck drill cycle definition)
X10.0 Y10.0                   (drill at position 1)
X30.0 Y10.0                   (drill at position 2)
X50.0 Y10.0                   (drill at position 3)
G80                            (cancel cycle)
```

Cycle types map to:
| IR cycle_type | G-code |
|---------------|--------|
| `drill` | `G81` |
| `peck_drill` | `G83` |
| `spot_drill` | `G81` (shallow) |
| `bore` | `G85` / `G86` |
| `tap` | `G84` |

Milling cycles are generally **not** available as canned cycles in standard G-code. The G-code post-processors only use canned cycles for drilling patterns.

#### Heidenhain TNC

Heidenhain has a rich set of canned cycles defined with `CYCL DEF` and called with `CYCL CALL`:

```
CYCL DEF 200 DRILLING ~
  Q200=2    ;SET-UP CLEARANCE ~
  Q201=-25  ;DEPTH ~
  Q206=200  ;FEED RATE FOR PLUNGING ~
  Q202=5    ;INFEED DEPTH ~
  Q210=0    ;DWELL TIME AT TOP ~
  Q203=0    ;SURFACE COORDINATE ~
  Q204=10   ;2ND SET-UP CLEARANCE
L X+10 Y+10 FMAX M99
L X+30 Y+10 FMAX M99
L X+50 Y+10 FMAX M99
```

Heidenhain also supports milling cycles that G-code controllers typically don't have as canned cycles:

| IR cycle_type | Heidenhain CYCL DEF |
|---------------|---------------------|
| `drill` | `CYCL DEF 200 DRILLING` |
| `peck_drill` | `CYCL DEF 203 UNIVERSAL DRILLING` |
| `spot_drill` | `CYCL DEF 200 DRILLING` |
| `bore` | `CYCL DEF 201 REAMING` |
| `tap` | `CYCL DEF 207 RIGID TAPPING` |
| `contour_pocket` | `CYCL DEF 251 RECTANGULAR POCKET` / `CYCL DEF 252 CIRCULAR POCKET` / `CYCL DEF 273 OCM POCKET` |
| `contour_profile` | `CYCL DEF 25 CONTOUR TRAIN` / `CYCL DEF 270 CONTOUR DATA` |
| `face_mill` | `CYCL DEF 232 FACE MILLING` |

**Heidenhain contour cycles** are particularly powerful — they can reference a contour definition (defined with `CYCL DEF 270 CONTOUR DATA` + `CYCL DEF 271 OCM CONTOUR DATA`) and the controller handles roughing, finishing, and depth stepping internally. When the user selects controller cycles for a pocket/profile on a Heidenhain machine, the post-processor can emit the contour definition + cycle call instead of thousands of explicit moves.

### Operation-Level Cycle Control

Operations get an optional `use_canned_cycle` field:

```python
@dataclass
class Operation:
    # ...existing fields...
    use_canned_cycle: bool  # Default: False. When True, compiler emits cycle blocks
                            # if the post-processor supports the relevant cycle type.
```

When `use_canned_cycle` is `True`:
1. The compiler checks if the post-processor supports the relevant cycle type
2. If supported: emits `CYCLE_DEFINE` + `CYCLE_CALL` blocks
3. If not supported: falls back to explicit moves (same as `use_canned_cycle=False`)

This gives the user explicit control — they can choose whether the CAM or the controller handles the pattern, and the system gracefully degrades when the target controller doesn't support the cycle.

### Implementation Phases for Cycles

Canned cycle support is planned for later phases:
- **Drilling cycles** (G81/G83, CYCL DEF 200/203): Implemented alongside drill operations in Phase 4
- **Heidenhain milling cycles** (CYCL DEF 251/252/273): Phase 5+, since they require contour definition output and are tightly coupled to the Heidenhain post-processor

## Optional Operations

Operations can be marked `optional=True` so the machine operator can skip them at runtime without editing the NC program. This is common for finishing passes, deburring, engraving, or chamfering that may not be needed on every part.

### Skip Levels

Each optional operation has a `skip_level` (1-9). Operations with the same skip level are skipped together. This lets the operator control groups independently:

| skip_level | Typical use |
|---|---|
| 1 | Finishing passes |
| 2 | Chamfer / deburring |
| 3 | Engraving / marking |
| 4-9 | User-defined |

### How Post-Processors Implement It

The NC compiler wraps optional operations in `OPTIONAL_SKIP_START` / `OPTIONAL_SKIP_END` blocks. Each post-processor translates these to its native skip mechanism.

#### Strategy 1: Block Delete (`/`)

The simplest and most portable approach. Every line within the optional section is prefixed with `/` (or `/2`, `/3`, etc. for multi-level). The operator toggles the "block delete" switch on the controller panel.

**Supported by**: Virtually all controllers (Fanuc, LinuxCNC, Haas, Grbl, Sinumerik, Heidenhain)

**Limitation**: On basic controllers, there's only one block delete switch — all `/` lines are either skipped or not. Controllers with multi-level block delete (`/1` through `/9`) can skip levels independently, which maps perfectly to our `skip_level`.

**Fanuc / LinuxCNC / Haas**:
```gcode
(Optional: Finish profile - Block delete level 1)
/ N0200 T2 M6 (3mm finish end mill)
/ N0210 S24000 M3
/ N0220 G0 X0 Y0
/ N0230 G0 Z5.000
/ N0240 G41 D02
/ N0250 G1 X10.000 Y0 Z-10.000 F600.000
...
/ N0300 G40
/ N0310 M5
```

With multi-level block delete (Haas, some Fanuc):
```gcode
/2 N0200 T2 M6 (chamfer mill)
/2 N0210 S12000 M3
...
```

**Grbl**:
```gcode
; Optional: Finish profile (toggle block delete to skip)
/ G0 X0 Y0
/ G0 Z5
/ G1 X10 Y0 Z-10 F600
...
```
Grbl supports single-level `/` only.

**Sinumerik**:
```gcode
; Optional: Finish profile
/1 N200 T2 M6
/1 N210 S24000 M3
...
```
Sinumerik supports `/1` through `/8`.

**Heidenhain**:
Heidenhain also supports `/` block delete:
```
/ 50 TOOL CALL 2 Z S24000
/ 51 L X+0 Y+0 FMAX M3
/ 52 L X+10 Y+0 Z-10 F600 RL
...
```

#### Strategy 2: Labels + Conditional Jumps

More flexible than block delete — the operator sets a variable (parameter/Q-parameter/R-parameter) to control which operations run. This allows runtime decisions without a hardware switch, and works well on controllers with limited block delete levels.

The post-processor declares which strategy it prefers via a property:

```python
class PostProcessor(ABC):
    # ...existing methods...

    @property
    def optional_skip_strategy(self) -> str:
        """
        How to implement optional operation skipping.
        "block_delete": Use / prefix (default, most portable)
        "jump": Use labels and conditional jumps (more flexible)
        "both": Emit both / prefix and jump structure (belt and suspenders)
        """
        return "block_delete"
```

**Heidenhain** (jump strategy):
```
; Check Q parameter — Q1=0 means skip finishing
FN 9: IF +Q1 EQU +0 GOTO LBL 10
50 TOOL CALL 2 Z S24000
51 L X+0 Y+0 FMAX M3
52 L X+10 Y+0 Z-10 F600 RL
...
59 L Z+25 FMAX
LBL 10
```
The operator sets `Q1=1` to run the finishing pass, `Q1=0` to skip it. This can be done from the controller panel or via a program header section.

**Sinumerik** (jump strategy):
```gcode
; R1=0 to skip finishing
IF R1==0 GOTOF SKIP_FINISH
N200 T2 M6
N210 S24000 M3
...
N300 M5
SKIP_FINISH:
```

**LinuxCNC** (jump strategy using O-codes):
```gcode
; #<_skip_finish> = 0 to skip
O100 IF [#<_skip_finish> EQ 0]
  O100 GOTO 999
O100 ENDIF
N200 T2 M6
...
O999 (skip target)
```

**Fanuc Macro B** (jump strategy):
```gcode
(Skip finishing if #500 = 0)
IF [#500 EQ 0] GOTO 100
N200 T2 M6
N210 S24000 M3
...
N100 (skip target)
```

#### Strategy Comparison

| Aspect | Block delete (`/`) | Conditional jump |
|---|---|---|
| Portability | Universal | Controller-specific syntax |
| Multi-level | Limited (1-9 depending on controller) | Unlimited (one variable per operation) |
| Runtime control | Hardware switch on panel | Set variable value |
| Program readability | Lines visually marked with `/` | Clear IF/GOTO structure |
| Nesting | Not possible | Possible (nested IFs) |
| Grbl support | Yes (single level) | No |

**Default**: Block delete is the default strategy for all post-processors. Jump strategy is an option on controllers that support it (Heidenhain, Sinumerik, LinuxCNC, Fanuc Macro B). The user can choose per-export in the export dialog.

### Compiler Behavior

When the compiler encounters an operation with `optional=True`:

1. Emit `OPTIONAL_SKIP_START` block with `skip_level` and a label name derived from the operation name
2. Emit all the operation's NCBlocks (tool change, spindle, toolpath, etc.)
3. Emit `OPTIONAL_SKIP_END` block

The post-processor's `format_blocks()` method handles these:
- For block delete: sets an internal flag, prefixes subsequent lines with `/N` until `OPTIONAL_SKIP_END`
- For jumps: emits the conditional jump + label pair

### Safety Considerations

Skipping an operation at the machine must leave the machine in a safe state:

- The compiler inserts a **safe Z retract** before `OPTIONAL_SKIP_START` so the tool is clear regardless of whether the section runs
- After `OPTIONAL_SKIP_END`, the compiler re-establishes state: cancels any compensation (`G40`), cancels any active cycle (`G80`/`CYCL OFF`), rapids to safe Z
- If the skipped operation included a tool change, the next operation must handle the fact that the tool may or may not have been changed — the compiler inserts a redundant `TOOL_CHANGE` after the optional section if needed

## Example Output

Given a simple facing operation on a 100x80mm stock, the LinuxCNC post-processor would generate:

```gcode
%
(CAMproject - My Part)
(Generated: 2026-03-20)
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
