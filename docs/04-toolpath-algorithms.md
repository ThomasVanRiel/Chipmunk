# Toolpath Generation Algorithms

## Overview

All 2.5D toolpath generation follows the same pattern:

1. **Get 2D contour** at target Z depth (via mesh slicing or direct 2D import)
2. **Apply tool compensation** (offset polygon by tool radius)
3. **Generate passes** at multiple Z depths (depth strategy)
4. **Order segments** to minimize rapids
5. **Emit toolpath segments** (rapid, linear, arc)

## Mesh Slicing (3D → 2D)

### `toolpath/slicer.py`

For STL/STEP inputs, we need to extract 2D cross-sections at specific Z heights.

**Algorithm**:
1. Use `trimesh.Trimesh.section(plane_origin, plane_normal)` to cut the mesh with a horizontal plane at the given Z height
2. The result is a `trimesh.path.Path3D` — a set of 3D line segments lying in the cutting plane
3. Project to 2D by dropping the Z coordinate → `Path2D`
4. Convert `Path2D` entities (lines, arcs) to Shapely `Polygon`/`MultiPolygon`
5. Cache slices by Z height (same Z is often queried multiple times)

**Edge cases**:
- Mesh may have multiple disconnected bodies → `MultiPolygon`
- Holes in the cross-section (e.g., a tube) → Shapely polygon with interior rings
- Very thin features may produce degenerate slices → filter by minimum area

```python
def slice_mesh(mesh: trimesh.Trimesh, z: float, tolerance: float = 0.001) -> MultiPolygon:
    """Slice a mesh at height z, return 2D cross-section as Shapely geometry."""
    ...
```

## Polygon Offset

### `toolpath/offset.py`

Wraps pyclipr (Clipper2) for computing tool-compensated profiles.

**Operations**:
- **Outward offset**: For outside profiling — expand the polygon by tool radius
- **Inward offset**: For inside profiling and pocketing — shrink the polygon by tool radius
- **Multi-step inward offset**: For pocketing — repeatedly shrink by stepover until nothing remains

```python
def offset_polygon(
    polygon: MultiPolygon,
    distance: float,            # Positive = outward, negative = inward
    join_type: str = "round",   # "round", "square", "miter"
    tolerance: float = 0.01,
) -> MultiPolygon:
    """Offset a polygon by the given distance."""
    ...

def iterative_offset(
    polygon: MultiPolygon,
    stepover: float,
    max_iterations: int = 1000,
) -> list[MultiPolygon]:
    """
    Repeatedly offset inward by stepover until the polygon vanishes.
    Returns list of offset polygons from outermost to innermost.
    Used for contour-parallel pocketing.
    """
    ...
```

**Why pyclipr over Shapely.buffer()?** Shapely's buffer works but is slower for the iterative offset case (pocketing can require hundreds of offset steps). pyclipr uses Clipper2 which is specifically optimized for polygon offsetting and boolean operations. Shapely is used for the geometry model; pyclipr is an implementation detail inside offset.py.

## Depth Strategy

### `toolpath/depth_strategy.py`

Determines the Z levels for multi-pass operations.

```python
def compute_depth_passes(
    start_depth: float,         # Usually 0.0 (stock top)
    final_depth: float,         # Negative value (into stock)
    depth_per_pass: float,      # Maximum cut depth per pass
) -> list[float]:
    """
    Return list of Z heights for each pass, from shallowest to deepest.

    Example: start=0, final=-10, depth_per_pass=3
    Returns: [-3.0, -6.0, -9.0, -10.0]

    The last pass may be shallower than depth_per_pass.
    """
    ...
```

**Even distribution option**: Instead of fixed depth per pass with a shallow final pass, optionally distribute passes evenly:
- `start=0, final=-10, depth_per_pass=3` → 4 passes at `-2.5, -5.0, -7.5, -10.0`
- This gives more consistent cutting forces

## Facing Operation

### `toolpath/facing.py`

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

### `toolpath/profile.py`

Cuts along the outline of a shape — like tracing the contour with the tool.

**Algorithm**:
1. Get 2D contour at target Z depth
2. Offset by tool radius:
   - Outside profile: offset outward by `tool_diameter / 2`
   - Inside profile: offset inward by `tool_diameter / 2`
   - On-line profile: no offset (center of tool follows the contour)
3. Determine cut direction:
   - **Climb milling** (recommended for CNC): tool moves CCW around outside profiles, CW around inside profiles
   - **Conventional milling**: opposite
4. For each depth pass:
   a. Rapid to safe Z
   b. Rapid to lead-in start point
   c. Lead-in move (arc or straight ramp into material)
   d. Follow the offset contour
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

### `toolpath/pocket.py`

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

Islands (raised features inside the pocket that should not be cut) are represented as holes in the Shapely polygon.

```python
# Pocket boundary with island
pocket = Polygon(
    shell=[(0,0), (100,0), (100,80), (0,80)],
    holes=[[(30,30), (50,30), (50,50), (30,50)]]
)
```

The polygon offset operation naturally handles islands — as the offset shrinks the outer boundary, it also expands the island boundaries (since they are interior rings). Eventually the growing islands merge with the shrinking boundary, splitting the pocket into sub-pockets.

### Rest Machining

After roughing with a large tool, a smaller tool cleans up the remaining material:

1. Generate the ideal pocket boundary offset by the small tool radius
2. Generate the area already cleared by the large tool (offset by large tool radius)
3. Subtract: remaining material = small_tool_area - large_tool_area
4. Generate toolpaths only in the remaining material areas

This is a Phase 2+ optimization — the initial implementation uses a single tool per pocket.

## Drill Operation (Phase 4)

### `toolpath/drill.py`

Generates drilling cycles for point features (holes).

**Algorithm**:
1. Identify drill points (from DXF circles, SVG circles, or user-placed points)
2. For each point, generate a drill cycle:
   - **Simple drill**: Rapid down to R-plane, feed to depth, rapid retract
   - **Peck drill**: Feed to partial depth, retract to clear chips, repeat
   - **Spot drill**: Shallow drill for center marking

**Cycle representation**: Drill cycles are represented as NCBlocks. Post-processors that support canned cycles (G81/G83) output them; post-processors that don't (e.g., Grbl) expand them to equivalent G0/G1 moves.

## Segment Ordering

### `toolpath/ordering.py`

After generating individual toolpath loops/segments, they need to be ordered to minimize total rapid travel.

**Current approach**: Nearest-neighbor heuristic
1. Start from the current position (or machine home)
2. Find the nearest unvisited toolpath segment start point
3. Execute that segment
4. Repeat from step 2

This is a variant of the Traveling Salesman Problem. Nearest-neighbor gives reasonable results for typical CAM scenarios. A more sophisticated approach (2-opt improvement) can be added later if needed.

## Safe Z Heights

All operations use two Z safety heights:
- **Rapid height** (clearance plane): Z height for rapid moves between features (typically 5-10mm above stock). Used for moves within the same operation.
- **Safe height** (retract plane): Z height for moves between operations or to home (typically 25-50mm above stock).

These are project-level settings, overridable per operation.
