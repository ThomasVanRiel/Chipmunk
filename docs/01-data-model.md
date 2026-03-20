# Data Model

## Overview

The data model is centered around a `Project` which contains everything needed to go from imported geometry to NC code output. All types live in `src/camproject/core/`.

## Type Hierarchy

```
Project
├── parts: list[PartGeometry]
├── tools: list[Tool]              (references global ToolLibrary, editable per-project)
├── operations: list[Operation]
│     ├── wcs: WorkCoordinateSystem      (per-operation, supports multi-setup)
│     ├── stock: StockDefinition | None  (optional, per-operation)
│     └── toolpath: Toolpath (generated, not saved)
│           └── segments: list[ToolpathSegment]
└── history: CommandHistory        (undo/redo)
```

## Core Types

### Project

The root container. Serializable to/from JSON for project save/load.

```python
@dataclass
class Project:
    name: str
    units: Units                      # MM or INCH
    parts: list[PartGeometry]
    tools: list[Tool]
    operations: list[Operation]
    history: CommandHistory            # Undo/redo log
    post_processor_id: str | None     # Default post-processor for this project
```

### WorkCoordinateSystem

Defines the machine work coordinate origin and orientation relative to the part for a given operation. This is a full coordinate frame (position + rotation), not just an offset. WCS is **per-operation**, enabling multi-setup parts (e.g., flip the part, different WCS for the back side).

Operations sharing the same setup can reference the same WCS values. When operations are grouped into setups (future tree structure), the group defines the WCS and child operations inherit it.

```python
@dataclass
class WorkCoordinateSystem:
    origin: tuple[float, float, float]      # XYZ position relative to part origin
    rotation: tuple[float, float, float]    # ABC rotation angles (degrees)
    work_offset: str                        # G54-G59 (which offset register on the machine)
```

**Default**: Part origin as-imported (0, 0, 0 position, no rotation, G54). The user can change this by:
- Clicking a point/edge/face on the part to place the origin
- Typing XYZ coordinates and ABC rotation manually
- Selecting a preset (top center, corner, etc.)

### StockDefinition (Optional)

Defines the raw material. **Not required** for toolpath generation — the operator knows their stock. Useful later for:
- Toolpath optimization (avoiding air cuts)
- Material removal simulation
- Rest machining (knowing what material remains)
- Heidenhain `BLK FORM` output

```python
@dataclass
class StockDefinition:
    shape: StockShape                 # BOX or CYLINDER
    width: float | None               # X dimension (box)
    height: float | None              # Y dimension (box)
    depth: float | None               # Z dimension (box)
    diameter: float | None            # For cylindrical stock
    length: float | None              # For cylindrical stock

class StockShape(Enum):
    BOX = "box"
    CYLINDER = "cylinder"
```

### PartGeometry

Wraps either a 3D mesh or a 2D contour set. Provides a uniform interface for toolpath generation.

```python
@dataclass
class PartGeometry:
    id: str                           # UUID
    name: str                         # Display name (usually filename)
    source_format: str                # "stl", "dxf", "svg", "step"

    # 3D representation (from STL/STEP)
    mesh: trimesh.Trimesh | None

    # 2D representation (from DXF/SVG, or sliced from mesh)
    contours_2d: shapely.MultiPolygon | None

    # Transform applied to the imported geometry
    transform: np.ndarray             # 4x4 homogeneous transform matrix

    # Provenance — where the geometry came from (see 08-integrations.md)
    provenance: PartProvenance | None  # None for legacy/unknown imports

    # Update history — tracks geometry changes over time (see 09-part-update.md)
    update_history: list[PartUpdate]  # Append-only log of updates

    def get_contour_at_z(self, z: float) -> shapely.MultiPolygon:
        """Slice the 3D mesh at height z, or return 2D contours."""
        ...

    def bounding_box(self) -> BoundingBox:
        """Axis-aligned bounding box in world coordinates."""
        ...
```

**Design note**: The `get_contour_at_z()` method is the key abstraction that lets toolpath generators work identically on both 3D meshes and 2D drawings. For 2D imports, this returns the same contours regardless of Z. For 3D meshes, it slices using `trimesh.section()` and converts the result to Shapely geometry.

### Tool

Defines a cutting tool's physical geometry and recommended cutting data. The recommended values auto-populate operation parameters when the tool is selected, but the user can always override per-operation.

Tools are stored in a **global library** (persistent across projects). When added to a project, they are copied in and can be edited per-project without affecting the global library.

```python
class ToolType(Enum):
    END_MILL = "end_mill"
    BALL_NOSE = "ball_nose"
    V_BIT = "v_bit"
    DRILL = "drill"

class CoolantMode(Enum):
    OFF = "off"
    FLOOD = "flood"               # M8
    MIST = "mist"                 # M7
    THROUGH_TOOL = "through_tool" # M88 or controller-specific

@dataclass
class Tool:
    id: str
    name: str                         # e.g. "6mm 2-flute end mill"
    type: ToolType
    diameter: float                   # Cutting diameter
    flute_length: float               # Maximum depth of cut
    total_length: float
    num_flutes: int

    # Recommended cutting data (auto-populates operations, user can override)
    recommended_feed_rate: float | None       # mm/min
    recommended_plunge_rate: float | None
    recommended_spindle_speed: float | None   # RPM
    recommended_depth_per_pass: float | None
    recommended_stepover: float | None        # Fraction of diameter (0.0-1.0)
    recommended_coolant: CoolantMode | None
```

**Workflow**: When the user selects a tool for an operation, the operation's cutting parameters are pre-filled from the tool's recommended values. The user can then adjust any value. Once overridden, changing the tool doesn't overwrite the user's edits — only empty/unset fields are populated.

### ToolLibrary

Global tool library stored server-side, independent of any project.

```python
@dataclass
class ToolLibrary:
    tools: list[Tool]
```

API: `GET/POST/PUT/DELETE /api/tools` (global library, separate from project tools).

### Operation

Represents a machining operation. Each subclass knows how to generate its toolpath.

```python
class OperationType(Enum):
    FACING = "facing"
    PROFILE = "profile"
    POCKET = "pocket"
    DRILL = "drill"

class CutDirection(Enum):
    CLIMB = "climb"             # Preferred for CNC
    CONVENTIONAL = "conventional"

class ProfileSide(Enum):
    OUTSIDE = "outside"
    INSIDE = "inside"
    ON = "on"                   # Cut on the line (no offset)

class CompensationMode(Enum):
    CAM = "cam"                 # CAM computes offset toolpath (tool center follows pre-offset path)
    CONTROLLER = "controller"   # CAM outputs geometry path, controller applies G41/G42

@dataclass
class Operation:
    id: str
    name: str
    type: OperationType
    enabled: bool

    # Optional at the machine (see 03-nc-and-postprocessors.md "Optional Operations")
    optional: bool                # If True, operator can skip this at the machine
    skip_level: int               # 1-9, maps to block delete level or jump variable

    # References
    geometry_id: str              # Which PartGeometry to machine
    tool_id: str                  # Which Tool to use

    # Coordinate system & stock (per-operation, supports multi-setup parts)
    wcs: WorkCoordinateSystem     # WCS for this operation
    stock: StockDefinition | None # Optional, for optimization/simulation

    # Cutting parameters (pre-filled from tool recommendations, user can override)
    feed_rate: float              # mm/min or in/min — XY cutting feed
    plunge_rate: float            # Feed rate for Z plunges
    spindle_speed: float          # RPM
    depth_per_pass: float         # Maximum Z step per pass
    start_depth: float            # Z start (usually 0 = WCS Z zero)
    final_depth: float            # Z end (negative = into material)
    coolant: CoolantMode          # Coolant mode for this operation

    # Machine control
    stop_before: str | None       # "M0" (mandatory stop) or "M1" (optional stop) before this op
    stop_after: str | None        # "M0" or "M1" after this op

    # Type-specific parameters (set based on operation type)
    # Facing
    stepover: float | None        # As fraction of tool diameter (0.0-1.0)

    # Profile
    profile_side: ProfileSide | None
    cut_direction: CutDirection | None
    compensation: CompensationMode | None  # CAM offset vs controller G41/G42
    tabs_enabled: bool
    tab_width: float | None
    tab_height: float | None
    lead_in_radius: float | None

    # Pocket
    pocket_stepover: float | None
    pocket_strategy: str | None   # "contour_parallel" or "zigzag"

    # Canned cycles (future — see 03-nc-and-postprocessors.md)
    use_canned_cycle: bool        # Default: False. Emit cycle blocks if post-processor supports it.

    # Computed output
    toolpath: Toolpath | None     # Generated, not serialized
```

**Alternative considered**: Using separate dataclasses per operation type (FacingOperation, ProfileOperation, etc.). Decided against it because a single type is simpler for serialization, API contracts, and the operations panel UI. Type-specific fields are simply None when not applicable.

### Cutter Compensation: CAM vs Controller

Operations that involve tool radius offset (profile, pocket walls) support two compensation modes:

- **`CAM` mode** (default): The CAM software computes the offset toolpath. The NC code contains the tool center coordinates — the controller simply follows them. This is the safest and most portable approach. Suitable for roughing where exact tool diameter matters less, and for controllers without cutter compensation support (e.g., Grbl).

- **`Controller` mode**: The CAM software outputs the **geometry path** (the actual part contour). The NC code includes `G41` (left offset) or `G42` (right offset) commands, and the controller applies the tool radius from its tool table at runtime. This allows the operator to fine-tune the tool diameter on the machine (e.g., to account for tool wear) without regenerating toolpaths. Ideal for finishing passes where dimensional accuracy matters.

**Implications for toolpath generation**:
- In `CAM` mode: `toolpath/profile.py` offsets the contour by `tool_diameter / 2` and emits tool-center coordinates.
- In `Controller` mode: `toolpath/profile.py` emits the original contour coordinates. The NC compiler adds `G41`/`G42` with a `D` word referencing the tool offset register. A lead-in move is **required** (controller needs a linear move to ramp into compensation).

**Implications for NC compilation**:
- `nc/compiler.py` checks the operation's compensation mode. For `Controller` mode, it emits:
  - `G41 D01` (or `G42 D01`) before the contour
  - The contour path at geometry coordinates (not offset)
  - `G40` to cancel compensation after the contour
- Post-processors translate these IR blocks to their native syntax:
  - G-code controllers: `G41 D01` / `G42 D01` / `G40`
  - Heidenhain: `RL` / `RR` / `R0` appended to the move line (compensation is part of the move command, not a separate block — the Heidenhain post-processor merges the COMP block with the next LINEAR/ARC block)

**Typical usage pattern**:
- Roughing pass: `CompensationMode.CAM` with a stock-to-leave allowance
- Finishing pass: `CompensationMode.CONTROLLER` so the operator can dial in the exact dimension

### Toolpath

The result of toolpath generation. A sequence of segments that describe the tool's movement.

```python
class MoveType(Enum):
    RAPID = "rapid"               # G0 — fast non-cutting move
    LINEAR = "linear"             # G1 — straight cutting move
    ARC_CW = "arc_cw"            # G2 — clockwise arc
    ARC_CCW = "arc_ccw"          # G3 — counter-clockwise arc

@dataclass
class ToolpathSegment:
    move_type: MoveType
    x: float
    y: float
    z: float

    # Arc-specific (None for rapid/linear)
    i: float | None               # Arc center X offset from start
    j: float | None               # Arc center Y offset from start

    # Cutting parameters (None for rapids)
    feed_rate: float | None

@dataclass
class Toolpath:
    operation_id: str
    segments: list[ToolpathSegment]

    # Computed metadata
    total_distance: float         # Total tool travel
    cutting_distance: float       # Feed moves only
    estimated_time: float         # Based on feed rates
    bounding_box: BoundingBox
```

### Supporting Types

```python
class Units(Enum):
    MM = "mm"
    INCH = "inch"

@dataclass
class BoundingBox:
    min_x: float
    min_y: float
    min_z: float
    max_x: float
    max_y: float
    max_z: float
```

## Undo/Redo (Command History)

Every mutation to the project is recorded as a command in a persistent, append-only history log. This enables unlimited undo/redo that survives across sessions.

```python
@dataclass
class Command:
    id: str
    timestamp: str                    # ISO 8601
    type: str                         # e.g. "add_operation", "update_operation", "delete_tool", ...
    description: str                  # Human-readable, e.g. "Added facing operation"
    forward_patch: dict               # JSON patch to apply (redo)
    reverse_patch: dict               # JSON patch to undo

@dataclass
class CommandHistory:
    commands: list[Command]           # Full history, oldest first
    cursor: int                       # Current position (index of next redo)
```

**How it works**:
- Every API mutation (add/edit/delete operation, tool, WCS, etc.) creates a `Command` with both forward and reverse patches
- **Undo**: Apply `reverse_patch` of command at `cursor - 1`, decrement cursor
- **Redo**: Apply `forward_patch` of command at `cursor`, increment cursor
- New mutations after an undo clear the redo tail (commands after cursor are discarded)
- History is persisted as part of the project — undo works across sessions
- User can clear history via `DELETE /api/project/history` if the project file grows too large

**What's recorded**: Only data mutations (operations, tools, WCS, part transforms). Toolpath generation is not recorded (toolpaths are regenerated, not undone). Viewport state (camera position, panel layout) is not recorded.

## Operation Duplication

Operations can be duplicated to create a copy with all parameters preserved. This is a common workflow for creating a finishing pass from a roughing pass — duplicate, then adjust depth, feed, and compensation mode.

`POST /api/project/operations/{id}/duplicate` creates a new operation with:
- All parameters copied from the source
- Name suffixed with " (copy)"
- Inserted immediately after the source in the operation list
- No toolpath (must be generated separately)

## Serialization & Auto-Persistence

The project is **auto-persisted** on every change — there is no save button. The server writes the project state after each mutation. The project file is a `.camproj` JSON file:

```json
{
  "version": "1.0",
  "name": "My Project",
  "units": "mm",
  "parts": [
    {
      "id": "...",
      "name": "bracket.stl",
      "source_format": "stl",
      "source_file": "bracket.stl",
      "transform": [[1,0,0,0], [0,1,0,0], [0,0,1,0], [0,0,0,1]]
    }
  ],
  "tools": [ ... ],
  "operations": [ ... ],
  "history": { "commands": [...], "cursor": 42 }
}
```

The actual geometry files (STL, DXF, etc.) are referenced by path, not embedded. The `.camproj` file stores enough information to re-import them. A future enhancement could bundle everything into a ZIP archive.

Toolpaths are **not** saved — they are regenerated from operations. This keeps the save file small and avoids stale toolpath data.

## Mesh Data for Frontend

When the frontend requests mesh data for Three.js rendering, the backend returns a compact JSON format:

```json
{
  "vertices": [x0, y0, z0, x1, y1, z1, ...],  // flat Float32 array
  "normals": [nx0, ny0, nz0, ...],              // per-vertex normals
  "indices": [i0, i1, i2, ...]                   // triangle indices
}
```

This maps directly to Three.js `BufferGeometry` attributes for efficient rendering.

Toolpath data for visualization:

```json
{
  "segments": [
    {"type": "rapid", "x": 10, "y": 20, "z": 5},
    {"type": "linear", "x": 10, "y": 20, "z": -2, "feed": 500},
    ...
  ]
}
```

The frontend renders rapids as dashed red lines and feed moves as solid colored lines (blue for XY moves, green for plunge moves).
