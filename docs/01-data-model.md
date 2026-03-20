# Data Model

## Overview

The data model is centered around a `Project` which contains everything needed to go from imported geometry to NC code output. All types live in `src/core/`.

## Type Hierarchy

```
Project
├── project_type: ProjectType            (3D or 2.5D — set at creation, immutable)
├── parts: Vec<PartGeometry>             (B-rep shapes for 3D, wires/faces for 2.5D)
├── tools: Vec<Tool>                     (references global ToolLibrary, editable per-project)
├── setups: Vec<Setup>                   (groups of operations sharing WCS/stock/clearance)
│     ├── wcs: WorkCoordinateSystem
│     ├── stock: Option<StockDefinition>
│     ├── clearance_height: f64
│     └── operations: Vec<Operation>
│           ├── setup_id: Uuid           (parent setup, inherits WCS/stock/clearance)
│           └── toolpath: Option<Toolpath>
│                 └── segments: Vec<ToolpathSegment>
└── history: CommandHistory              (undo/redo)
```

## Core Types

### Project

The root container. Serializable to/from JSON for project save/load.

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ProjectType {
    ThreeD,     // 3D projects: STEP/STL input, B-rep geometry, 3D viewport
    TwoHalfD,  // 2.5D projects: DXF/SVG input, 2D wire/profile geometry, top-down view
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub project_type: ProjectType,         // Set at creation, immutable
    pub units: Units,
    pub parts: Vec<PartGeometry>,
    pub tools: Vec<Tool>,
    pub setups: Vec<Setup>,
    pub history: CommandHistory,
    pub post_processor_id: Option<String>,  // Default post-processor for this project
}
```

**Project type** determines:
- **3D**: Accepts STEP and STL imports. Parts are B-rep solids. Frontend shows a 3D viewport with face selection.
- **2.5D**: Accepts DXF and SVG imports. Parts are 2D wires and faces. Frontend shows a top-down 2D view. Depth comes from operation parameters, not geometry.

A project is one type or the other — set at creation time, not changeable afterward. A physical part that needs both 3D milling and 2D engraving uses two separate projects.

### WorkCoordinateSystem

Defines the machine work coordinate origin and orientation relative to the part for a given operation. This is a full coordinate frame (position + rotation), not just an offset. WCS is **per-operation**, enabling multi-setup parts (e.g., flip the part, different WCS for the back side).

Operations sharing the same setup can reference the same WCS values. When operations are grouped into setups (future tree structure), the group defines the WCS and child operations inherit it.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkCoordinateSystem {
    pub origin: [f64; 3],          // XYZ position relative to part origin
    pub rotation: [f64; 3],        // ABC rotation angles (degrees)
    pub work_offset: String,       // G54-G59 (which offset register on the machine)
}
```

**Default**: Part origin as-imported (0, 0, 0 position, no rotation, G54). The user can change this by:
- Clicking a point/edge/face on the part to place the origin
- Typing XYZ coordinates and ABC rotation manually
- Selecting a preset (top center, corner, etc.)

### Setup

Groups operations that share the same workholding. Defines the WCS, stock, and clearance height — child operations inherit these but can override any value.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setup {
    pub id: Uuid,
    pub name: String,                          // e.g. "Setup 1 — Top"
    pub wcs: WorkCoordinateSystem,
    pub stock: Option<StockDefinition>,
    pub clearance_height: f64,                 // Safe Z for rapids within this setup
}
```

**Clearance height** is the only safety height the user configures. It defines the Z for rapid moves between operations within the setup. The post-processor inserts full retractions to clearance height between setups when generating multi-setup programs. Retract between passes within an operation is handled by the toolpath generator (typically stock top + a small margin).

**WCS inheritance**: Operations within a setup inherit its WCS, stock, and clearance height. An operation can override any inherited value by setting it explicitly. If the setup's WCS changes, all operations that haven't overridden it update automatically.

NC export is typically per-setup (one program per fixture).

### StockDefinition (Optional)

Defines the raw material. **Not required** for toolpath generation — the operator knows their stock. Useful later for:
- Toolpath optimization (avoiding air cuts)
- Material removal simulation
- Rest machining (knowing what material remains)
- Heidenhain `BLK FORM` output

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockDefinition {
    pub shape: StockShape,
    pub width: Option<f64>,        // X dimension (box)
    pub height: Option<f64>,       // Y dimension (box)
    pub depth: Option<f64>,        // Z dimension (box)
    pub diameter: Option<f64>,     // For cylindrical stock
    pub length: Option<f64>,       // For cylindrical stock
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StockShape {
    Box,
    Cylinder,
}
```

### PartGeometry

Wraps an OpenCascade B-rep shape. For 3D projects this is a solid or shell from STEP/STL. For 2.5D projects this is a collection of wires and faces from DXF/SVG.

The B-rep is the primary representation — all operations (slicing, face selection, bounding box) work on it directly. Triangle meshes are generated on demand for the frontend viewport.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartGeometry {
    pub id: Uuid,
    pub name: String,                      // Display name (usually filename)
    pub source_format: String,             // "stl", "dxf", "svg", "step"

    // Primary representation — OpenCascade B-rep shape
    // Not serialized to JSON — persisted as a separate .brep file
    #[serde(skip)]
    pub shape: TopoDS_Shape,

    // Cached face metadata (serializable, for API/frontend)
    pub faces: Vec<FaceInfo>,

    // File reference for the persisted B-rep
    pub brep_file: String,                 // e.g. "bracket.brep" — alongside the .camproj

    // Transform applied to the imported geometry
    pub transform: [[f64; 4]; 4],          // 4x4 homogeneous transform matrix

    // Provenance — where the geometry came from (see 08-integrations.md)
    pub provenance: Option<PartProvenance>,

    // Update history — tracks geometry changes over time (see 09-part-update.md)
    pub update_history: Vec<PartUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceInfo {
    pub id: u32,                           // Stable face index within the shape
    pub surface_type: SurfaceType,
    pub normal: Option<[f64; 3]>,          // Outward normal (planar faces only)
    pub area: f64,
    pub bounding_box: BoundingBox,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SurfaceType {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    BSpline,
    Other,
}
```

The `PartGeometry` type provides key methods:

```rust
impl PartGeometry {
    /// Section the B-rep shape at height Z.
    /// Returns exact curves (lines, arcs, B-splines) — not polyline approximations.
    /// Uses BRepAlgoAPI_Section (shape vs. plane intersection).
    /// For 2.5D parts, returns the same 2D geometry regardless of Z.
    pub fn section_at_z(&self, z: f64) -> Vec<TopoDS_Edge> { ... }

    /// Get all faces of a specific surface type.
    /// Useful for feature detection (e.g., Cylinder faces = holes).
    pub fn faces_by_type(&self, surface_type: SurfaceType) -> Vec<&FaceInfo> { ... }

    /// Access the underlying B-rep face by index.
    pub fn face(&self, face_id: u32) -> TopoDS_Face { ... }

    /// Tessellate the B-rep for display. Generates a triangle mesh on demand.
    /// `deflection` controls mesh density (lower = finer, default 0.1mm).
    /// Returns face_ids per triangle for face selection in the frontend.
    pub fn tessellate(&self, deflection: f64) -> TessellatedMesh { ... }

    /// Axis-aligned bounding box in world coordinates.
    pub fn bounding_box(&self) -> BoundingBox { ... }

    /// Get edges of a specific face as curves.
    pub fn face_edges(&self, face_id: u32) -> Vec<TopoDS_Edge> { ... }
}
```

**Design note**: `section_at_z()` replaces the old `get_contour_at_z()`. The key improvement is that it returns exact geometry — an arc in the model stays an arc in the cross-section, rather than being approximated as a polyline. For toolpath generators that use polygon offset (pockets), the exact edges are converted to `geo::MultiPolygon` at the toolpath stage. For profiles in controller compensation mode, exact arcs are preserved and emitted directly as `ArcCw`/`ArcCcw` segments.

### TessellatedMesh

Generated on demand from the B-rep shape for frontend display. Not stored — recomputed when needed.

```rust
#[derive(Debug, Clone)]
pub struct TessellatedMesh {
    pub vertices: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,     // Per-vertex normals
    pub indices: Vec<[u32; 3]>,      // Triangle indices
    pub face_ids: Vec<u32>,          // One per triangle — maps to FaceInfo.id
}
```

The `face_ids` array enables face selection in the frontend: raycasting identifies a triangle, the triangle's `face_id` identifies the B-rep face, and the backend can then read the face's exact geometry (normal, surface type, edges) for operations like orientation or WCS placement.

### Tool

Defines a cutting tool's physical geometry and recommended cutting data. The recommended values auto-populate operation parameters when the tool is selected, but the user can always override per-operation.

Tools are stored in a **global library** (persistent across projects). When added to a project, they are copied in and can be edited per-project without affecting the global library.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolType {
    EndMill,
    BallNose,
    VBit,
    Drill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoolantMode {
    Off,
    Flood,         // M8
    Mist,          // M7
    ThroughTool,   // M88 or controller-specific
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: Uuid,
    pub name: String,                              // e.g. "6mm 2-flute end mill"
    pub tool_number: u32,                          // Machine tool number (T1, T2, ...)
    pub machine: Option<String>,                   // Machine label — for multi-machine projects
    pub tool_type: ToolType,
    pub diameter: f64,                             // Cutting diameter
    pub flute_length: f64,                         // Maximum depth of cut
    pub total_length: f64,
    pub num_flutes: u32,

    // Recommended cutting data (auto-populates operations, user can override)
    pub recommended_feed_rate: Option<f64>,         // mm/min
    pub recommended_plunge_rate: Option<f64>,
    pub recommended_spindle_speed: Option<f64>,     // RPM
    pub recommended_depth_per_pass: Option<f64>,
    pub recommended_stepover: Option<f64>,          // Fraction of diameter (0.0-1.0)
    pub recommended_coolant: Option<CoolantMode>,
}
```

**Tool number and name**: The post-processor decides which identifier to use in NC output. G-code controllers use `tool_number` (`T1 M6`). Heidenhain can use either (`TOOL CALL 1` or `TOOL CALL "6MM_EM"`). Tool numbers are not required to be unique within a project — a project may span multiple machines. Uniqueness is enforced within the same `machine` value only.

**Workflow**: When the user selects a tool for an operation, the operation's cutting parameters are pre-filled from the tool's recommended values. The user can then adjust any value. Once overridden, changing the tool doesn't overwrite the user's edits — only empty/unset fields are populated.

### ToolLibrary

Global tool library stored server-side, independent of any project.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLibrary {
    pub tools: Vec<Tool>,
}
```

API: `GET/POST/PUT/DELETE /api/tools` (global library, separate from project tools).

### Operation

Represents a machining operation. Each type knows how to generate its toolpath.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    Facing,
    Profile,
    Pocket,
    Drill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CutDirection {
    Climb,          // Preferred for CNC
    Conventional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProfileSide {
    Outside,
    Inside,
    On,             // Cut on the line (no offset)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompensationMode {
    Cam,            // CAM computes offset toolpath (tool center follows pre-offset path)
    Controller,     // CAM outputs geometry path, controller applies G41/G42
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: Uuid,
    pub name: String,
    pub operation_type: OperationType,
    pub enabled: bool,

    // Optional at the machine (see 03-nc-and-postprocessors.md "Optional Operations")
    pub optional: bool,            // If true, operator can skip this at the machine
    pub skip_level: u8,            // 1-9, maps to block delete level or jump variable

    // References
    pub setup_id: Uuid,            // Parent setup (inherits WCS, stock, clearance height)
    pub geometry_id: Uuid,         // Which PartGeometry to machine
    pub tool_id: Uuid,             // Which Tool to use

    // Overrides (None = inherit from setup)
    pub wcs_override: Option<WorkCoordinateSystem>,
    pub stock_override: Option<StockDefinition>,
    pub clearance_height_override: Option<f64>,

    // Cutting parameters (pre-filled from tool recommendations, user can override)
    pub feed_rate: f64,            // mm/min or in/min — XY cutting feed
    pub plunge_rate: f64,          // Feed rate for Z plunges
    pub spindle_speed: f64,        // RPM
    pub depth_per_pass: f64,       // Maximum Z step per pass
    pub start_depth: f64,          // Z start (usually 0 = WCS Z zero)
    pub final_depth: f64,          // Z end (negative = into material)
    pub coolant: CoolantMode,

    // Machine control
    pub stop_before: Option<String>,  // "M0" (mandatory stop) or "M1" (optional stop)
    pub stop_after: Option<String>,

    // Type-specific parameters (set based on operation type)
    // Facing
    pub stepover: Option<f64>,                    // Fraction of tool diameter (0.0-1.0)

    // Profile
    pub profile_side: Option<ProfileSide>,
    pub cut_direction: Option<CutDirection>,
    pub compensation: Option<CompensationMode>,   // CAM offset vs controller G41/G42
    pub tabs_enabled: bool,
    pub tab_width: Option<f64>,
    pub tab_height: Option<f64>,
    pub lead_in_radius: Option<f64>,

    // Pocket
    pub pocket_stepover: Option<f64>,
    pub pocket_strategy: Option<String>,          // "contour_parallel" or "zigzag"
    pub pocket_entry: Option<String>,             // "plunge" (default), "helix", or "ramp"

    // Canned cycles (future — see 03-nc-and-postprocessors.md)
    pub use_canned_cycle: bool,    // Default: false. Emit cycle blocks if post-processor supports it.

    // Computed output
    #[serde(skip)]
    pub toolpath: Option<Toolpath>,
}
```

**Alternative considered**: Using separate structs per operation type (FacingOperation, ProfileOperation, etc.) or a Rust enum with per-variant data. Decided on a single struct because it simplifies serialization, API contracts, and the operations panel UI. Type-specific fields are simply `None` when not applicable.

### Cutter Compensation: CAM vs Controller

Operations that involve tool radius offset (profile, pocket walls) support two compensation modes:

- **`Cam` mode** (default): The CAM software computes the offset toolpath. The NC code contains the tool center coordinates — the controller simply follows them. This is the safest and most portable approach. Suitable for roughing where exact tool diameter matters less, and for controllers without cutter compensation support (e.g., Grbl).

- **`Controller` mode**: The CAM software outputs the **geometry path** (the actual part contour). The NC code includes `G41` (left offset) or `G42` (right offset) commands, and the controller applies the tool radius from its tool table at runtime. This allows the operator to fine-tune the tool diameter on the machine (e.g., to account for tool wear) without regenerating toolpaths. Ideal for finishing passes where dimensional accuracy matters.

**Implications for toolpath generation**:
- In `Cam` mode: `toolpath/profile.rs` offsets the contour by `tool_diameter / 2` and emits tool-center coordinates.
- In `Controller` mode: `toolpath/profile.rs` emits the original contour coordinates. The NC compiler adds `G41`/`G42` with a `D` word referencing the tool offset register. A lead-in move is **required** (controller needs a linear move to ramp into compensation).

**Implications for NC compilation**:
- `nc/compiler.rs` checks the operation's compensation mode. For `Controller` mode, it emits:
  - `G41 D01` (or `G42 D01`) before the contour
  - The contour path at geometry coordinates (not offset)
  - `G40` to cancel compensation after the contour
- Post-processors translate these IR blocks to their native syntax:
  - G-code controllers: `G41 D01` / `G42 D01` / `G40`
  - Heidenhain: `RL` / `RR` / `R0` appended to the move line (compensation is part of the move command, not a separate block — the Heidenhain post-processor merges the COMP block with the next LINEAR/ARC block)

**Typical usage pattern**:
- Roughing pass: `CompensationMode::Cam` with a stock-to-leave allowance
- Finishing pass: `CompensationMode::Controller` so the operator can dial in the exact dimension

### Toolpath

The result of toolpath generation. A sequence of segments that describe the tool's movement.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveType {
    Rapid,      // G0 — fast non-cutting move
    Linear,     // G1 — straight cutting move
    ArcCw,      // G2 — clockwise arc
    ArcCcw,     // G3 — counter-clockwise arc
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolpathSegment {
    pub move_type: MoveType,
    pub x: f64,
    pub y: f64,
    pub z: f64,

    // Arc-specific (None for rapid/linear)
    pub i: Option<f64>,     // Arc center X offset from start
    pub j: Option<f64>,     // Arc center Y offset from start

    // Cutting parameters (None for rapids)
    pub feed_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toolpath {
    pub operation_id: Uuid,
    pub segments: Vec<ToolpathSegment>,

    // Computed metadata
    pub total_distance: f64,      // Total tool travel
    pub cutting_distance: f64,    // Feed moves only
    pub estimated_time: f64,      // Based on feed rates
    pub bounding_box: BoundingBox,
}
```

### Supporting Types

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Units {
    Mm,
    Inch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}
```

## Undo/Redo (Command History)

Every mutation to the project is recorded as a command in a persistent, append-only history log. This enables unlimited undo/redo that survives across sessions.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: Uuid,
    pub timestamp: String,                 // ISO 8601
    pub command_type: String,              // e.g. "add_operation", "update_operation", "delete_tool"
    pub description: String,               // Human-readable, e.g. "Added facing operation"
    pub forward_patch: serde_json::Value,  // JSON patch to apply (redo)
    pub reverse_patch: serde_json::Value,  // JSON patch to undo
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistory {
    pub commands: Vec<Command>,            // Full history, oldest first
    pub cursor: usize,                     // Current position (index of next redo)
}
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
  "project_type": "3d",
  "units": "mm",
  "parts": [
    {
      "id": "550e8400-...",
      "name": "bracket.step",
      "source_format": "step",
      "brep_file": "bracket.brep",
      "faces": [ { "id": 0, "surface_type": "plane", "normal": [0, 0, 1], "area": 500.0, ... }, ... ],
      "transform": [[1,0,0,0], [0,1,0,0], [0,0,1,0], [0,0,0,1]]
    }
  ],
  "tools": [ ... ],
  "setups": [
    {
      "id": "...",
      "name": "Setup 1 — Top",
      "wcs": { "origin": [0,0,0], "rotation": [0,0,0], "work_offset": "G54" },
      "stock": { "shape": "box", "width": 110, "height": 85, "depth": 25 },
      "clearance_height": 50.0
    }
  ],
  "operations": [ ... ],
  "history": { "commands": [...], "cursor": 42 }
}
```

**Geometry persistence**: B-rep shapes are saved as separate `.brep` files (OpenCascade native format) alongside the `.camproj` file. The `brep_file` field references the filename. On project load, OpenCascade reads the `.brep` file directly into a `TopoDS_Shape` — no re-import from the original source file needed.

```
my_project/
├── my_project.camproj       # Project JSON (metadata, tools, operations, history)
├── bracket.brep             # B-rep shape for part "bracket"
└── housing.brep             # B-rep shape for part "housing"
```

Toolpaths are **not** saved — they are regenerated from operations. This keeps the save file small and avoids stale toolpath data.

## Mesh Data for Frontend

For 3D projects, the backend tessellates the B-rep shape on demand and returns a compact JSON format for Three.js rendering. The `face_ids` array maps each triangle back to the B-rep face it belongs to, enabling face selection.

```json
{
  "vertices": [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, ...],
  "normals": [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, ...],
  "indices": [0, 1, 2, ...],
  "face_ids": [0, 0, 0, 1, 1, 2, ...]
}
```

This maps directly to Three.js `BufferGeometry` attributes. The `face_ids` are stored as a custom buffer attribute and used during raycasting to identify which B-rep face was clicked.

Tessellation quality is controlled by a `deflection` parameter (lower = finer mesh). Default: 0.1mm — suitable for typical hobby/small-shop parts.

For 2.5D projects, there is no 3D mesh — the frontend renders 2D wires and faces in a top-down view.

Toolpath data for visualization:

```json
{
  "start_position": {"x": 0, "y": 0, "z": 25},
  "segments": [
    {"type": "rapid", "x": 10, "y": 20, "z": 5},
    {"type": "linear", "x": 10, "y": 20, "z": -2, "feed": 500},
    ...
  ]
}
```

The frontend renders rapids as dashed red lines and feed moves as solid colored lines (blue for XY moves, green for plunge moves).
