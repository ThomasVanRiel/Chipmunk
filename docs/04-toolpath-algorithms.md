# Toolpath Generation Algorithms

## Overview

All 2.5D toolpath generation follows the same pattern:

1. **Get 2D contour** at target Z depth (via B-rep sectioning or direct 2D import)
2. **Apply tool compensation** (offset polygon by tool radius)
3. **Generate passes** at multiple Z depths (depth strategy)
4. **Order segments** to minimize rapids
5. **Emit toolpath segments** (rapid, linear, arc)

## B-Rep Sectioning (3D → 2D)

### `toolpath/slicer.rs`

For STEP/STL inputs (3D projects), we extract 2D cross-sections at specific Z heights using OpenCascade's `BRepAlgoAPI_Section` — intersecting the B-rep shape with a horizontal plane.

**Algorithm**:
1. Construct a horizontal plane at the given Z height
2. Use `BRepAlgoAPI_Section` to intersect the `TopoDS_Shape` with the plane → produces `TopoDS_Wire` (exact curves)
3. Convert wires to `geo::Polygon`/`geo::MultiPolygon` for offset operations (arcs tessellated at this stage)
4. Cache slices by Z height (same Z is often queried multiple times)

**Key advantage over mesh slicing**: The section produces exact geometry — lines stay as lines, arcs stay as arcs, B-splines stay as B-splines. This enables arc-preserving profiles in controller compensation mode (see Profile section).

**Edge cases**:
- Shape may have multiple disconnected bodies → `MultiPolygon`
- Holes in the cross-section (e.g., a tube) → polygon with interior rings
- Very thin features may produce degenerate slices → filter by minimum area

```rust
pub fn section_at_z(shape: &TopoDS_Shape, z: f64) -> Vec<TopoDS_Wire> {
    /// Section a B-rep shape at height z, return exact 2D wires.
}

pub fn section_to_polygons(shape: &TopoDS_Shape, z: f64, tolerance: f64) -> MultiPolygon {
    /// Section at z and convert to geo polygons for offset operations.
    /// Arcs are tessellated during conversion.
}
```

For 2.5D projects (DXF/SVG), no sectioning is needed — the imported wires/faces are already 2D geometry. Depth comes from operation parameters.

## Polygon Offset

### `toolpath/offset.rs`

Wraps geo-clipper (Clipper2 bindings) for computing tool-compensated profiles.

**Operations**:
- **Outward offset**: For outside profiling — expand the polygon by tool radius
- **Inward offset**: For inside profiling and pocketing — shrink the polygon by tool radius
- **Multi-step inward offset**: For pocketing — repeatedly shrink by stepover until nothing remains

```rust
pub fn offset_polygon(
    polygon: &MultiPolygon,
    distance: f64,             // Positive = outward, negative = inward
    join_type: JoinType,       // Round, Square, Miter
    tolerance: f64,
) -> MultiPolygon {
    /// Offset a polygon by the given distance.
}

pub fn iterative_offset(
    polygon: &MultiPolygon,
    stepover: f64,
    max_iterations: usize,
) -> Vec<MultiPolygon> {
    /// Repeatedly offset inward by stepover until the polygon vanishes.
    /// Returns list of offset polygons from outermost to innermost.
    /// Used for contour-parallel pocketing.
}
```

**Why geo-clipper over geo's buffer?** The `geo` crate's buffer support is limited. geo-clipper provides Clipper2 bindings which are specifically optimized for polygon offsetting and boolean operations — critical for pocketing where hundreds of offset steps may be needed. `geo` is used for the geometry model; geo-clipper is an implementation detail inside `offset.rs`.

## Depth Strategy

### `toolpath/depth_strategy.rs`

Determines the Z levels for multi-pass operations.

```rust
pub fn compute_depth_passes(
    start_depth: f64,       // Usually 0.0 (stock top)
    final_depth: f64,       // Negative value (into stock)
    depth_per_pass: f64,    // Maximum cut depth per pass
) -> Vec<f64> {
    /// Return list of Z heights for each pass, from shallowest to deepest.
    ///
    /// Example: start=0, final=-10, depth_per_pass=3
    /// Returns: [-3.0, -6.0, -9.0, -10.0]
    ///
    /// The last pass may be shallower than depth_per_pass.
}
```

**Even distribution option**: Instead of fixed depth per pass with a shallow final pass, optionally distribute passes evenly:
- `start=0, final=-10, depth_per_pass=3` → 4 passes at `-2.5, -5.0, -7.5, -10.0`
- This gives more consistent cutting forces

## Facing Operation

### `toolpath/facing.rs`

Removes material from the top of the stock to create a flat reference surface.

**Algorithm** (zigzag/raster):
1. Define the facing boundary: stock bounding box expanded by tool radius (for full coverage)
2. Generate parallel lines across the boundary spaced by stepover (`tool_diameter * stepover_fraction`)
3. Alternate direction on each line (zigzag — avoids repositioning rapids)
4. For each Z pass: emit plunge at start, then the zigzag pattern

```
Pass direction →
Y ↑
  │  ┌────────────────────┐
  │  │ ──────────────────→ │
  │  │ ←────────────────── │
  │  │ ──────────────────→ │
  │  │ ←────────────────── │
  │  └────────────────────┘
  └──────────────────────────→ X
```

**Parameters**:
- `stepover`: Fraction of tool diameter (typically 0.6-0.8 for facing)
- `final_depth`: Usually very shallow (0.5-1mm) unless leveling rough stock
- `margin`: Extra distance beyond stock boundary for full coverage
- `direction`: Along X or along Y

## Profile Operation

### `toolpath/profile.rs`

Cuts along the outline of a shape — like tracing the contour with the tool.

**Algorithm**:
1. Get 2D contour at target Z depth
2. Apply tool compensation based on `CompensationMode`:

   **Cam mode** (default):
   - Outside profile: offset contour outward by `tool_diameter / 2`
   - Inside profile: offset contour inward by `tool_diameter / 2`
   - On-line profile: no offset
   - Emitted coordinates are the tool center path

   **Controller mode**:
   - No geometric offset is applied — emit the original contour coordinates
   - NC compiler adds `G41` (climb/left) or `G42` (conventional/right) with `D` register
   - NC compiler adds `G40` after the contour to cancel compensation
   - Lead-in move is **mandatory** (controller needs a linear approach move to ramp into compensation — the offset is not applied during the `G41`/`G42` block itself, but takes effect on the next move)

3. Determine cut direction:
   - **Climb milling** (recommended for CNC): tool moves CCW around outside profiles, CW around inside profiles
   - **Conventional milling**: opposite
4. For each depth pass:
   a. Rapid to safe Z
   b. Rapid to lead-in start point
   c. Lead-in move (arc or straight ramp into material)
   d. Follow the contour (offset or geometry path depending on compensation mode)
   e. Lead-out move
   f. Rapid to safe Z
5. If tabs are enabled, insert tab segments (raise Z to tab height at specified intervals)

**Lead-in/Lead-out** (reduces witness marks at entry/exit):
```
                ╭─── lead-in arc
               ╱
    ──────────●─────────── profile path
               ╲
                ╰─── lead-out arc
```

**Tabs** (hold-down tabs for sheet cutting):
```
Profile path with tabs:

    ───╱╲───────────╱╲───────────╱╲───
       tab          tab          tab

Tab = segment where Z raises to (final_depth + tab_height)
```

**Parameters**:
- `profile_side`: outside / inside / on-line
- `cut_direction`: climb / conventional
- `lead_in_radius`: Arc radius for entry (0 = straight plunge)
- `tabs_enabled`, `tab_width`, `tab_height`, `tab_count`
- `finishing_pass`: Optional separate light pass at full depth after roughing

## Pocket Operation

### `toolpath/pocket.rs`

Clears material from an enclosed area — the most algorithmically complex 2.5D operation.

### Strategy 1: Contour-Parallel (Offset)

**Algorithm**:
1. Get the pocket boundary polygon
2. Offset inward by tool radius to get the first cutting pass (tool center follows this path)
3. Continue offsetting inward by stepover amount until the polygon vanishes
4. For each depth pass, execute all offset loops from outermost to innermost
5. Connect loops with linking moves

```
Original pocket boundary:
┌──────────────────┐
│                  │
│   Island         │
│   ┌────┐         │
│   │    │         │
│   └────┘         │
│                  │
└──────────────────┘

Offset passes (tool center paths):
┌─ ─ ─ ─ ─ ─ ─ ─ ┐
  ┌──────────────┐
  │ ┌──────────┐ │
  │ │          │ │
  │ │ ┌──┐     │ │
  │ │ │Is│     │ │   (offsets curve around island)
  │ │ └──┘     │ │
  │ └──────────┘ │
  └──────────────┘
└ ─ ─ ─ ─ ─ ─ ─ ─┘
```

**Loop ordering**: Process from outermost to innermost. This ensures each pass removes a consistent width of material.

**Linking moves**: Between offset loops, the tool must move from one loop to the next. Options:
- **Retract and rapid**: Safe but slow (rapid up, move over, plunge down)
- **Direct link**: Stay at cutting depth and move directly to next loop start (faster but may leave witness mark)
- **Ramp link**: Gradually ramp down while moving to next loop (compromise)

### Strategy 2: Zigzag (Raster)

**Algorithm**:
1. Get the pocket boundary polygon
2. Offset inward by tool radius
3. Generate parallel lines across the pocket, spaced by stepover
4. Clip each line to the (offset) pocket boundary
5. Connect clipped line segments in zigzag order

```
Zigzag pocket clearing:
┌──────────────────┐
│ ─────────────→   │
│ ←─────────────   │
│ ─────────────→   │
│ ←─ ┌────┐ ──    │  (lines broken around island)
│ ── │    │ ──→   │
│ ←─ └────┘ ──    │
│ ─────────────→   │
│ ←─────────────   │
└──────────────────┘
```

**Advantages of zigzag**: Simpler algorithm, more predictable cutting forces, better for wide shallow pockets.

**Advantages of contour-parallel**: Better surface finish on pocket walls, handles complex shapes and islands more naturally.

### Island Handling

Islands (raised features inside the pocket that should not be cut) are represented as holes in the `geo::Polygon`.

```rust
// Pocket boundary with island
let pocket = Polygon::new(
    LineString::from(vec![(0.0, 0.0), (100.0, 0.0), (100.0, 80.0), (0.0, 80.0), (0.0, 0.0)]),
    vec![
        LineString::from(vec![(30.0, 30.0), (50.0, 30.0), (50.0, 50.0), (30.0, 50.0), (30.0, 30.0)]),
    ],
);
```

The polygon offset operation naturally handles islands — as the offset shrinks the outer boundary, it also expands the island boundaries (since they are interior rings). Eventually the growing islands merge with the shrinking boundary, splitting the pocket into sub-pockets.

### Rest Machining

After roughing with a large tool, a smaller tool cleans up the remaining material:

1. Generate the ideal pocket boundary offset by the small tool radius
2. Generate the area already cleared by the large tool (offset by large tool radius)
3. Subtract: remaining material = small_tool_area - large_tool_area
4. Generate toolpaths only in the remaining material areas

This is a Phase 2+ optimization — the initial implementation uses a single tool per pocket.

### Cutter Compensation for Pockets

Pocketing interacts with cutter compensation differently than profiling:

- **Interior clearing passes**: Always use `Cam` mode — controller compensation doesn't make sense for parallel offset loops in the pocket interior.
- **Final wall pass**: The outermost contour-parallel pass (closest to the pocket wall) can optionally use `Controller` mode. This is implemented as a separate finishing pass:
  1. Rough the pocket with `Cam` mode, leaving stock-to-leave on walls
  2. Run a single-pass profile along the pocket boundary with `Controller` mode (`G41`/`G42`)

This allows the operator to fine-tune pocket dimensions on the machine. The `compensation` field on the operation controls whether this wall finishing pass uses `Cam` or `Controller` mode.

## Drill Operation (Phase 4)

### `toolpath/drill.rs`

Generates drilling cycles for point features (holes).

**Algorithm**:
1. Identify drill points (from DXF circles, SVG circles, or user-placed points)
2. For each point, generate a drill cycle:
   - **Simple drill**: Rapid down to R-plane, feed to depth, rapid retract
   - **Peck drill**: Feed to partial depth, retract to clear chips, repeat
   - **Spot drill**: Shallow drill for center marking
   - **Bore**: Feed to depth, optional dwell at bottom, feed retract
   - **Tap**: Feed to depth at pitch-synchronized feed, reverse retract

**Dual output**: The toolpath generator always produces explicit moves (rapid/linear segments). When `use_canned_cycle == true`, the NC compiler additionally emits `CycleDefine` + `CycleCall` blocks. The post-processor chooses which form to output based on its `supported_cycles` set:

- **G-code controllers** (Fanuc, LinuxCNC, Haas): `G81`/`G83`/`G84`/`G85` with position-only lines, `G80` to cancel
- **Heidenhain**: `CYCL DEF 200`/`203`/`207` with `M99` cycle call on position lines
- **Grbl/Marlin**: Expanded explicit moves (no canned cycle support)

See `03-nc-and-postprocessors.md` for full cycle type mapping.

## Segment Ordering

### `toolpath/ordering.rs`

After generating individual toolpath loops/segments, they need to be ordered to minimize total rapid travel.

**Current approach**: Nearest-neighbor heuristic
1. Start from the current position (or machine home)
2. Find the nearest unvisited toolpath segment start point
3. Execute that segment
4. Repeat from step 2

This is a variant of the Traveling Salesman Problem. Nearest-neighbor gives reasonable results for typical CAM scenarios. A more sophisticated approach (2-opt improvement) can be added later if needed.

## Safe Z Heights

Operations use the **clearance height** defined on their parent setup (with optional per-operation override). This is the Z height for rapid moves between features within the same operation (typically 5-10mm above stock).

Between setups, the NC compiler inserts full retraction sequences (see `03-nc-and-postprocessors.md`) to ensure safe clearance before the next setup's work offset takes effect.
