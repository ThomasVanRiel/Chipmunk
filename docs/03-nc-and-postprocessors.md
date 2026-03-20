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
    PROGRAM_END = "program_end"     # M30/M2
    SET_UNITS = "set_units"         # G20/G21
    SET_WORK_OFFSET = "set_work_offset"  # G54-G59
    SET_PLANE = "set_plane"         # G17/G18/G19
    SET_MODE = "set_mode"           # G90/G91 (absolute/incremental)
    SET_FEED_MODE = "set_feed_mode" # G94/G95 (per-minute/per-rev)

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

    def generate(self, blocks: list[NCBlock], context: ProgramContext) -> str:
        """
        Generate complete NC code from a block list.
        Usually not overridden — subclasses customize via format_block/preamble/postamble.
        """
        lines = self.preamble(context)
        for block in blocks:
            line = self.format_block(block)
            if line is not None:
                lines.append(line)
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
