# Chipmunk — Design Document

A CLI-first CAM tool that generates NC code for CNC milling machines. Machining jobs are defined in YAML job files that reference SVG or DXF geometry; NC code is exported through pluggable Lua post-processors.

---

## Architecture

```
  CLI: chipmunk [job.yaml]
         │  calls directly
         ▼
  ┌──────────────────────────────────┐
  │         Core Library             │
  │  core/ → toolpath/ → nc/ → Lua  │
  │  io/ (SVG, DXF, YAML)           │
  └──────────────────────────────────┘
```

The CLI calls library functions directly — no HTTP, no server process. A REST API is planned as a peer-level thin shell over the same library, not a wrapper around the CLI.

### Data Flow

```
YAML job file (geometry: path/to/file.svg)
  → io/: parse geometry, group by stroke color
  → core/: Tool, Setup, Operation structs
  → toolpath/: generate segments (rapid, linear, arc, drill point)
  → nc/: compile to NCBlock IR (controller-neutral)
  → nc/bridge: Lua VM → post-processor → NC string
  → .H / .nc / .gcode file
```

### Module Dependency Rules

```
cli/       → core/, toolpath/, nc/, io/
api/       → core/, toolpath/, nc/, io/
core/      → (no internal deps — only geo, nalgebra, serde)
toolpath/  → core/
nc/        → core/
io/        → core/
```

`core/`, `toolpath/`, `nc/`, `io/` are pure computational modules — no axum, no clap. They are independently testable. `cli/` and `api/` are thin adapters.

---

## Design Principles

1. **CLI first, API as peer.** The CLI is the primary interface — direct hardware feedback with no server overhead. The API is a peer consumer of the same library.

2. **SVG color workflow.** Operations are selected by SVG stroke color. Each color maps to a full operation configuration in the YAML job file. Circles become drill points; closed paths become profiles, pockets, or facing areas.

3. **Controller agnosticism.** Toolpaths are abstract segment sequences. These compile to a controller-neutral IR. Only the final Lua post-processor step formats machine-specific output.

4. **Trust the operator.** Only error on physically impossible geometry (tool wider than pocket, depth exceeds part). Never warn about aggressive feeds or deep cuts.

5. **Never infer.** If a required parameter is missing or a tool ID cannot be resolved, hard error to stderr, exit 1. No silent defaults.

6. **Plugin post-processors.** Lua modules embedded at compile time. User post-processors are `.lua` files in the config directory. Fresh Lua VM per generation call.

---

## Data Model

### Project Structure

```
Project
├── project_type: 3D or 2.5D (set at creation, immutable)
├── parts: Vec<PartGeometry>
├── tools: Vec<Tool>
├── setups: Vec<Setup>
│     ├── wcs: WorkCoordinateSystem (origin, rotation, work_offset)
│     ├── stock: Option<StockDefinition>
│     ├── clearance_height: f64
│     └── operations: Vec<Operation>
└── history: CommandHistory (undo/redo via JSON patches)
```

**3D projects** accept STEP/STL imports. Parts are B-rep solids. **2.5D projects** accept DXF/SVG imports. Parts are 2D wires and faces. Depth comes from operation parameters.

### Setup

Groups operations that share the same workholding. Defines WCS, stock, and clearance height. Child operations inherit these but can override any value.

**One YAML = one setup** in the CLI workflow. Multiple setups use separate YAML files. The `wcs_marker_color` field in YAML identifies a circle whose center defines the WCS origin; if unset, origin defaults to SVG coordinate origin.

### Stock

Optional. Not required for toolpath generation — the operator knows their stock. Needed later for optimization (avoid air cuts), simulation, rest machining, and Heidenhain `BLK FORM` output. Box or cylinder shape.

### Tool

Tools carry physical geometry (diameter, flute length, type) and recommended cutting data (feed, speed, coolant) that auto-populate operations when selected. User can always override per-operation.

**Tool library** resolves from up to four sources, highest priority first:

1. Hardcoded inline on the operation (no ID, all params written directly)
2. Inline `tools:` section in the job YAML
3. `tools.yaml` next to the job file
4. `~/.config/chipmunk/tools.yaml` (global)

On ID collision, higher-priority source wins. Missing tool reference = hard error.

**Tool numbers** are scoped per `machine` value — not globally unique. Post-processor decides whether to call by number or name.

### Operation

A single struct for all operation types (Facing, Profile, Pocket, Drill). Type-specific fields are `None` when not applicable. This simplifies serialization and API contracts.

Key parameters shared across types: feed_rate, plunge_rate, spindle_speed, depth_per_pass, start_depth, final_depth, coolant.

**Drill operations** support two geometry sources (mutually exclusive):

- `color:` — circles of that stroke color in the geometry file become drill points
- `points:` — explicit XY coordinates in the YAML

Both missing or both present = hard error.

### Cutter Compensation

Operations support two modes:

- **CAM mode** (default): software computes offset toolpath, NC contains tool center coordinates. Portable and safe.
- **Controller mode**: NC contains geometry path + G41/G42 (or RL/RR). Operator can fine-tune dimensions at the machine. Requires a lead-in move. Ideal for finishing passes.

Typical pattern: rough with CAM mode + stock-to-leave, finish with controller mode.

### Optional Operations

Operations can be marked optional with a skip level (1–9). Post-processors implement via block delete (`/` prefix) or conditional jumps, depending on the target machine. The compiler inserts safe Z retract before/after optional sections.

### Toolpath

A sequence of segments: Rapid, Linear, ArcCw, ArcCcw. Segments carry XYZ coordinates, optional arc center offsets (i, j), and optional feed rate. Computed metadata includes total distance, cutting distance, and estimated time.

### Parallel Toolpath Generation

Operations are independent of each other — each operation's toolpath depends only on its own parameters and geometry. Toolpath generation for all operations runs in parallel, each on its own thread. Results are collected and ordered by operation sequence before NC compilation. NC compilation itself is sequential (the output order matters).

**Future constraint: simulation-driven rest machining.** When stock simulation is implemented, each operation's toolpath can be generated against the actual remaining stock (computed by simulating all prior operations), not just the original stock shape. This means operations become sequentially dependent — the toolpath generator receives the simulated stock state as input. This is more accurate than conventional rest machining (which only compares tool diameters) because it accounts for the actual material removal of every previous pass. Until stock simulation exists, all operations are independent and fully parallelizable.

---

## Three-Layer NC Generation

1. **Toolpath** — pure geometry segments produced by operation-specific generators
2. **NCBlock IR** — controller-neutral program blocks (adds spindle, tools, coolant, compensation, cycles, optional skips)
3. **Post-processor** — Lua module formats to machine-specific output

### NCBlock IR

The atomic unit of NC output. Each block type represents one logical instruction. The IR includes: Comment, Rapid, Linear, ArcCw, ArcCcw, ToolChange, SpindleOn, SpindleOff, CoolantOn, CoolantOff, Dwell, Stop, OptionalStop, ProgramEnd, SetUnits, SetWorkOffset, SetPlane, SetMode, SetFeedMode, CompLeft, CompRight, CompOff, CycleDefine, CycleCall, CycleOff, OptionalSkipStart, OptionalSkipEnd.

Blocks use typed enum variants with per-variant fields (not a HashMap).

### NC Compiler

The compiler separates the **envelope** (tool change, spindle, coolant, clearance — shared by all operations) from the **body** (operation-specific blocks).

**Compilation order:**

1. Program header (project name, date, safety line: units, absolute mode, plane, feed mode)
2. For each setup: work offset, then for each operation: envelope wrapping body
3. Full retraction between setups
4. Return to home, program end

The compiler tracks modal state and omits redundant values (e.g., consecutive linear moves at the same feed).

### Operation Body via compile_nc

Each operation type provides its body blocks via a `compile_nc` method on the `OperationType` trait. If `compile_nc` returns `None`, the compiler falls back to generic 1:1 conversion of toolpath segments to NCBlocks.

**Drilling** always returns `Some` from `compile_nc` because all drill strategies produce structurally different IR:

- Manual: Comment + Stop + Rapids (no canned cycle, works on every controller)
- Cycle-based (simple, peck, bore, tap): CycleDefine + CycleCall blocks when the post-processor supports the cycle type; returns `None` to fall back to explicit moves otherwise

**Milling** (facing, profile, pocket) typically returns `None` — the generic segment-to-block conversion is sufficient.

### Canned Cycles

The IR supports CycleDefine / CycleCall / CycleOff blocks. Toolpath generators always produce explicit moves as fallback. Canned cycles are preferred by default (`use_canned_cycle: true`) and emitted when the post-processor declares support.

Post-processors declare support via `M.supported_cycles = { "drill", "peck_drill", ... }`. Unsupported cycle types fall back to explicit moves automatically.
> Q: How will the function signature be passed to the Lua bridge?

| IR cycle_type | G-code | Heidenhain |
|---|---|---|
| `drill` | G81 | CYCL DEF 200 |
| `peck_drill` | G83 | CYCL DEF 203 |
| `bore` | G85/G86 | CYCL DEF 201 |
| `tap` | G84 | CYCL DEF 207 |

---

## Post-Processor System

Post-processors are Lua modules. A post-processor returns a table with `name`, `file_extension`, and a `generate(blocks, context)` function that produces the complete NC output string.

**Built-in** post-processors are embedded at compile time via `include_str!`. **User** post-processors are `.lua` files in `~/.config/chipmunk/postprocessors/`. A user file with the same name as a built-in overrides it.

A shared `base.lua` provides coordinate formatting helpers available to all post-processors.
> Q: Should we use `require("base")` instead of global functions? That will be clearer for the PP author. `base.lua` can contain bindings to rust algorithms? Is base the correct name?
> Maybe just send all possible parameters in a `LuaTable`? PP can pick what they need.

### Heidenhain TNC

Heidenhain uses conversational programming — a completely different syntax from G-code. The post-processor overrides `generate()` entirely rather than formatting block-by-block.

Key differences: mandatory line numbering, explicit sign on coordinates (`X+10`), `L X+n Y+n FMAX` for rapids, `TOOL CALL` instead of T/M6, `RL`/`RR`/`R0` appended to move lines for compensation, `CYCL DEF` for canned cycles with `M99` to trigger at each position.

### Capability Querying

Before compiling, the NC compiler queries the post-processor's Lua module for its capabilities (supported cycle types, optional skip strategy). This determines whether cycles or explicit moves are emitted.

---

## Operation Type System

Toolpath operation types are Rust structs implementing the `OperationType` trait. They are registered at compile time in a static registry — no runtime discovery.

The trait provides:

- `type_id()` / `display_name()` — identification
- `parameter_schema()` — describes accepted parameters (drives UI and validation)
- `validate()` — only errors for physically impossible situations
- `generate()` — produces explicit toolpath segments (used for visualization and as fallback)
- `compile_nc()` — optionally produces optimized NCBlocks (e.g., canned cycles for drilling)

**Adding a new operation type:**

1. Create struct in `toolpath/`, implement `OperationType`
2. Register in the static registry
3. Add integration tests

**Adding a new drill strategy** is lighter — add a variant to `DrillStrategy`, add the match arms in `generate()` and `compile_nc()`.

---

## Toolpath Algorithms (Rough ideas)

### B-Rep Sectioning (3D → 2D)

For STEP/STL inputs, extract 2D cross-sections at specific Z heights using OpenCascade's `BRepAlgoAPI_Section`. Returns exact geometry — arcs stay as arcs. Cache slices by Z height.

### Polygon Offset

Uses geo-clipper (Clipper2 bindings) for tool-compensated profiles. Supports outward offset (outside profiling), inward offset (inside profiling, pocketing), and iterative inward offset (contour-parallel pocketing).

### Depth Strategy

Computes Z levels for multi-pass operations. Supports both fixed depth-per-pass (with a potentially shallower final pass) and even distribution across passes.

### Facing

Zigzag/raster pattern across the stock bounding box (expanded by tool radius). Alternating direction on each line. Parameters: stepover, depth, margin, direction.

### Profile

Cuts along the outline of a shape. Supports outside/inside/on-line, climb/conventional, lead-in/lead-out arcs, and hold-down tabs. In controller compensation mode, emits geometry path with G41/G42 instead of offset path.

### Pocket

Clears material from an enclosed area. Two strategies:

- **Contour-parallel**: iterative inward offset, outermost to innermost, with linking moves between loops
- **Zigzag**: parallel lines clipped to pocket boundary

Handles islands (raised features) naturally through polygon interior rings. Entry options: plunge, helix, or ramp.

### Drill

Point-based. Strategies: manual (rapids only, operator drills by hand), simple (feed to depth, retract), peck (incremental feed with full retract), chip break (incremental feed with partial retract), bore (feed down and up at boring rate), tap (synchronized feed).

Drill point patterns: explicit `[x, y]` coordinates, or `circle_pattern`, `line_pattern`, `rect_pattern` — preserved through the IR for native post-processor pattern support.

### Segment Ordering

Nearest-neighbor heuristic to minimize rapid travel between toolpath segments.

---

## I/O

### YAML Job Files

The sole input for NC generation. Declares the post-processor, clearance height, units, geometry file path, tool definitions, and operations with their parameters and color bindings.

### SVG Import

Parse paths preserving stroke color. Circles become drill points (center + radius). Closed paths become contours. Group entities by color, match to operations by `color:` field.

### DXF Import

Parse entities preserving color (ACI or RGB). Same color grouping as SVG.

### WCS Marker

A circle drawn at a dedicated color (`wcs_marker_color` in YAML) whose center defines the WCS origin. Excluded from operation geometry.

---

## Implementation Phases

### Phase 1: Scaffolding + Manual Drill (Points in YAML)

First end-to-end workflow. Explicit XY points in YAML → manual drill toolpath → Heidenhain NC → run on machine. Minimal types: Tool, DrillParams, DrillStrategy::Manual, NCBlock (7 variants), YAML job parsing, mlua bridge, Heidenhain post-processor.

**Deliverable:** `chipmunk drill.yaml --output DRILL.H` → load on Heidenhain TNC → quill drill workflow works.

### Phase 2: Automatic Drill Cycles

Full drilling capability: peck, chip break, bore, tap. Canned cycles for Heidenhain and Haas. Still points-based. Adds: full DrillParams, NC IR cycle blocks, PostProcessorCapabilities, drill point patterns, optional operations, `--tool` filter, `--check` flag, Haas post-processor.

**Deliverable:** Multi-tool drill job → canned cycle NC output for Heidenhain and Haas.

### Phase 3: SVG/DXF Import + Color Workflow

Geometry-driven operations. SVG/DXF files provide drill points and contours via stroke color mapping. Adds: SVG import, DXF import, color-keyed grouping, WCS marker, `geometry:` field in YAML, `--geometry` override, `--color` filter, `--plot` flag for toolpath SVG visualization.

**Deliverable:** Geometry import works. Drill operations work with both `points:` and `color:`.

### Phase 4: 2.5D Milling

Profile, pocket, facing driven by SVG stroke colors. Adds: polygon offset, depth strategy, facing/profile/pocket generators, NC IR extensions (Linear, Arc, Coolant, Compensation), Heidenhain and Haas milling output.

**Deliverable:** `chipmunk job.yaml --output part.H` processes the full job. Draw in Inkscape, assign stroke colors, run command, machine the part.

### Phase Dependencies

```
Phase 1 (manual drill, points)     ← first hardware test
  → Phase 2 (automatic drill cycles) ← full drilling
    → Phase 3 (SVG/DXF import)       ← geometry-driven ops
      → Phase 4 (2.5D milling)       ← complete CLI workflow
```

---

## Planned: REST API

An axum server exposing the same library functions over HTTP. Peer to the CLI. Single-project model with auto-persistence. Endpoints for project CRUD, parts, tools, setups, operations, toolpath generation, NC export. WebSocket for real-time progress during generation.

Required before any frontend work.

## Planned: Browser Frontend

2D/3D viewport (Three.js for 3D, canvas for 2.5D). Tabbed sidebar: operations list, property editor, tool library, NC preview. Face/edge selection in the 3D viewport maps to B-rep faces via a `face_ids` array on the tessellated mesh. Toolpath visualization: dashed red rapids, solid colored feeds.

## Planned: OpenCascade Integration

B-rep geometry kernel via opencascade-rs (cxx.rs bindings to OCCT 7.8.1). Used for: STEP/STL import, B-rep persistence (.brep files), tessellation (face mesh + edge polylines), B-rep sectioning (shape vs. plane intersection for 2D cross-sections), shape introspection (face normals, surface types, bounding boxes, mass properties).

OCCT is not thread-safe — all computation runs in `spawn_blocking`. Custom cxx.rs bindings needed for: `BRepBuilderAPI_Sewing` (STL → shell), `BRepAdaptor_Curve` (arc detection in sections), `GCPnts_TangentialDeflection` (edge tessellation).

Geometry stored in millimeters, Z+ up, 1 micron tolerance.

## Planned: CAD Integrations

A `CADIntegration` trait for pluggable external CAD support. Integration layer sits above `io/` readers — handles authentication and API communication, passes retrieved geometry (STEP/STL bytes) to existing readers.

Priority order:

1. File import (Phase 1 baseline)
2. Watch folder — poll a directory for new/modified files; works with any CAD
3. Onshape — cloud REST API, OAuth2 or API keys, version tracking for refresh
4. FreeCAD — CLI subprocess (`freecadcmd`) to export .FCStd as STEP

Part provenance metadata (source type, document ID, version, timestamp) enables "refresh from CAD" and portability.

## Planned: Part Update Pipeline

When geometry is re-imported after CAD changes, preserve existing CAM setup:

1. **Geometry diff** — compare bounding box, volume, surface area, topology, centroid
2. **Registration** — ICP alignment to find the transform matching new geometry to old
3. **Change report** — classify changes (dimensions, added/removed features, origin shift)
4. **Operation audit** — check each operation against new geometry (depth, contour, stock, tool fit)
5. **User review** — present changes and recommendations before applying

Auto-adjustments (stock resize, depth scaling, origin re-alignment) require user confirmation. Never silently break a project.

## Planned: Undo/Redo

Every mutation recorded as a command with forward/reverse JSON patches. Persistent across sessions. User can clear history if project file grows large. Viewport state and toolpath generation are not recorded.

---

## Technology Stack

| Component | Choice | Rationale |
|---|---|---|
| Language | Rust | Performance-critical geometry; strong types; single binary |
| CLI | clap | Ergonomic subcommand + flag parsing |
| API | axum | Async HTTP + WebSocket; same binary via feature flag |
| Serialization | serde + serde_json + serde_yaml | JSON for projects; YAML for job files |
| Geometry kernel | opencascade-rs | B-rep, exact curves, STEP/STL import |
| 2D geometry | geo + geo-clipper | Polygon offset via Clipper2 |
| Linear algebra | nalgebra | Vectors, matrices, transforms |
| Post-processors | Lua 5.4 via mlua | ~300KB VM; designed for string formatting |
| HTTP client | reqwest | Async; for CAD integrations |
