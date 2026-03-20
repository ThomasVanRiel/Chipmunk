# Part Update & Geometry Change Handling

## Problem

When a part is re-imported after changes in CAD (dimension tweaks, added features, shifted origin), the CAM project should not be lost. Operations, tool assignments, and machining parameters represent significant user effort — often more than the geometry import itself.

A naive "replace geometry" destroys this work because:
- The new geometry may have shifted origin or orientation
- Bounding box changes mean stock dimensions are wrong
- Operations reference depths and contours that no longer match
- Added features (new holes, pockets) aren't automatically picked up
- Removed features leave orphaned operations

## Design Goals

1. **Never silently break a project** — always show the user what changed and what needs attention
2. **Preserve all operations by default** — an updated part should keep the existing CAM setup working
3. **Align geometry automatically when possible** — detect translation/rotation/scale and compensate
4. **Flag operations that may need review** — but don't delete them without user confirmation

## Update Pipeline

```
New geometry from source
        │
        ▼
┌─────────────────────┐
│ 1. Geometry Diff    │ Compare old vs new: bounding box, volume,
│                     │ surface area, topology (face/edge counts)
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ 2. Registration     │ Find the transform that best aligns
│    (Alignment)      │ new geometry to old geometry
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ 3. Change Report    │ Classify changes: dimensions, added/removed
│                     │ features, origin shift
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ 4. Operation Audit  │ Check each operation against new geometry:
│                     │ still valid? needs adjustment? invalid?
└─────────┬───────────┘
          ▼
┌─────────────────────┐
│ 5. User Review      │ Present changes and recommendations
│                     │ in the UI for confirmation
└─────────┘───────────┘
```

## Step 1: Geometry Diff

Compare the old and new geometry to understand what changed. This uses cheap metrics first, then more detailed analysis only if needed.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryDiff {
    // Bounding box changes
    pub old_bbox: BoundingBox,
    pub new_bbox: BoundingBox,
    pub bbox_delta: BoundingBox,          // Per-axis size difference

    // Volume/area (for meshes)
    pub old_volume: Option<f64>,
    pub new_volume: Option<f64>,
    pub old_surface_area: Option<f64>,
    pub new_surface_area: Option<f64>,

    // Topology (approximate — from mesh)
    pub old_face_count: usize,
    pub new_face_count: usize,
    pub topology_changed: bool,

    // Centroid shift
    pub old_centroid: [f64; 3],
    pub new_centroid: [f64; 3],
    pub centroid_shift: [f64; 3],

    // Classification
    pub change_type: ChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Identical,           // No meaningful change
    DimensionsOnly,      // Same topology, different size
    FeaturesAdded,       // More geometry (new holes, pockets, bosses)
    FeaturesRemoved,     // Less geometry
    FeaturesModified,    // Both added and removed
    OriginShifted,       // Same shape, different position
    MajorChange,         // Substantially different part
}
```

**Heuristics for classification**:
- `Identical`: Bounding box, volume, and surface area all within tolerance (e.g., 0.001mm)
- `DimensionsOnly`: Face count unchanged, volume/area changed, centroid shift proportional to size change
- `FeaturesAdded`: Face count increased, volume decreased (material removed = new pocket/hole) or increased (new boss)
- `OriginShifted`: Volume and surface area unchanged, centroid shifted
- `MajorChange`: Large volume change (>20%) or face count change (>30%)

## Step 2: Registration (Alignment)

Find the rigid transform (translation + rotation) that best aligns the new geometry to the old geometry. This ensures operations stay spatially correct even if the CAD origin shifted.

### Approach: ICP with Feature Matching

**For mesh-to-mesh (STL) alignment:**

1. **Bounding box pre-alignment**: Translate so centroids match as initial guess
2. **ICP (Iterative Closest Point)**: Refine alignment iteratively. Sample points from both meshes, find closest pairs, compute optimal rigid transform, repeat until convergence. The ICP implementation is custom Rust using nalgebra for the SVD-based transform computation.
3. **Validate**: If the residual error after ICP is below threshold, the alignment is good. If not, fall back to bounding-box alignment and flag for user review.

```rust
pub fn register_geometries(
    old_mesh: &TriMesh,
    new_mesh: &TriMesh,
    method: RegistrationMethod,
) -> RegistrationResult {
    /// Find the transform that aligns new_mesh to old_mesh's coordinate frame.
    /// Returns the 4x4 transform matrix and a confidence score.
}

#[derive(Debug, Clone)]
pub enum RegistrationMethod {
    Icp,
    BboxCenter,
    BboxCorner,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResult {
    pub transform: [[f64; 4]; 4],      // 4x4 matrix to apply to new geometry
    pub method_used: String,
    pub residual_error: f64,           // Mean point-to-point distance after alignment
    pub confidence: f64,               // 0.0-1.0, based on residual error relative to part size
}
```

**For 2D contour (DXF/SVG) alignment:**
- Simpler: align bounding box centers, then check if contours overlap using geo intersection-over-union (IoU)
- If IoU is high, alignment is good; if not, try rotating 90°/180°/270° and re-check

### User Override

The user can always manually adjust the alignment:
- Drag the new part into position in the viewport
- Pick three corresponding points on old/new geometry for point-based registration
- Enter a known offset (e.g., "the origin moved 10mm in X")

## Step 3: Change Report

After alignment, generate a human-readable report of what changed.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeReport {
    pub diff: GeometryDiff,
    pub registration: RegistrationResult,
    pub changes: Vec<ChangeItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeItem {
    pub category: String,             // "dimension", "feature", "origin", "topology"
    pub description: String,          // Human-readable, e.g. "Part is 5mm wider in X"
    pub severity: String,             // "info", "warning", "critical"
    pub affected_operations: Vec<Uuid>,
}
```

**Example change items**:
- `info`: "Part width (X) increased from 100mm to 105mm"
- `info`: "New feature detected: volume decreased by 2.3cm³ (possible new pocket)"
- `warning`: "Part origin shifted 10mm in Y — operations re-aligned automatically"
- `warning`: "Profile operation 'Outside contour' references geometry that changed shape"
- `critical`: "Part bounding box no longer fits within defined stock (105mm > 100mm stock width)"

## Step 4: Operation Audit

Check each existing operation against the new geometry. For each operation, determine a status.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationStatus {
    Ok,           // Operation is still valid, no changes needed
    Adjusted,     // Auto-adjusted (e.g., depths updated) — needs review
    Review,       // Probably still valid but geometry changed in the operation area
    Invalid,      // Operation can't work (e.g., deeper than new part, outside bounds)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationAudit {
    pub operation_id: Uuid,
    pub status: OperationStatus,
    pub issues: Vec<String>,          // Human-readable descriptions
    pub auto_adjustments: Vec<String>,
}
```

### Audit Checks

For each operation, run these checks against the new geometry:

**Depth checks**:
- Is `final_depth` still within the part? (Part might be thinner now)
- Is `start_depth` still at stock top? (Stock may need resizing)
- Auto-adjust: If part thickness changed proportionally, offer to scale depths

**Contour checks** (for profile/pocket):
- Slice the new mesh at the operation's Z depths
- Compare the new cross-section with the old one using geo:
  - Symmetric difference area → how much changed
  - Small change → `Ok` or `Review`
  - Large change or contour vanished → `Invalid`

**Stock checks**:
- Does the new bounding box still fit within the defined stock?
- If not → `Review` with suggestion to update stock dimensions

**Tool checks**:
- For pockets: is the tool diameter still smaller than the narrowest feature?
- For profiles: are there new tight corners the tool can't reach?

### Auto-Adjustments

Safe adjustments that can be applied automatically (with user confirmation):

- **Stock resize**: If the part grew, offer to expand stock to match (with margin)
- **Depth proportional scaling**: If the part went from 20mm to 22mm thick, scale all depths by 1.1x
- **Origin re-alignment**: Apply the registration transform to all operation coordinates

Risky adjustments that require explicit user action:
- Deleting operations that reference vanished features
- Changing operation types (e.g., a through-pocket became a blind pocket)
- Adding operations for new features

## Step 5: User Review UI

The update review is presented as a modal dialog/panel before the update is applied.

```
┌─────────────────────────────────────────────────┐
│  Part Update Review                             │
│                                                 │
│  Source: bracket.stl (Onshape, v47 → v52)       │
│  Change type: Dimensions + new features         │
│                                                 │
│  Changes:                                       │
│  i Part width (X): 100mm → 105mm               │
│  i New feature: volume decreased 2.3cm³         │
│  ! Origin shifted 10mm in Y (auto-corrected)   │
│  ! Stock too small: 100mm < 105mm in X         │
│                                                 │
│  Operations:                                    │
│  v Facing (top)           OK                    │
│  ~ Rough pocket           ADJUSTED              │
│    L Depth scaled 20mm → 22mm                   │
│  ! Outside profile        REVIEW                │
│    L Contour changed at Z=-10mm                 │
│  v Drill holes            OK                    │
│                                                 │
│  Suggestions:                                   │
│  [ ] Update stock to 110 x 80 x 25mm           │
│  [ ] Accept depth adjustments                   │
│  [ ] Regenerate all toolpaths after update      │
│                                                 │
│  [Accept & Update]  [Update without adjustments] │
│  [Cancel]                                       │
└─────────────────────────────────────────────────┘
```

The 3D viewport shows old geometry (transparent/wireframe) overlaid with new geometry (solid) so the user can visually verify the alignment.

## API

```
POST /api/project/parts/{id}/update
  Body: multipart file upload (new geometry) OR { "refresh_from_source": true }
  Response: ChangeReport + OperationAudit list (the update is NOT applied yet)

POST /api/project/parts/{id}/update/apply
  Body: {
    "accept_adjustments": true,     // Apply auto-adjustments
    "update_stock": true,           // Resize stock to fit
    "regenerate_toolpaths": true    // Re-generate all affected toolpaths
  }
  Response: Updated project state
```

The two-step API (preview → apply) ensures the user always reviews changes before they take effect.

## Data Model Additions

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartUpdate {
    pub timestamp: String,                         // ISO 8601
    pub previous_version: Option<String>,          // Provenance version before update
    pub new_version: Option<String>,               // Provenance version after update
    pub change_type: ChangeType,
    pub registration_transform: [[f64; 4]; 4],     // 4x4 matrix applied
    pub auto_adjustments: Vec<String>,             // What was auto-corrected
}
```

This gives a full audit trail of how the part evolved and what adjustments were made. If something goes wrong, the user can understand what happened.

## Edge Cases

**Part replaced with completely different geometry**: `MajorChange` classification. All operations flagged as `Review`. User must confirm they actually want to replace (not a wrong file).

**Part scaled uniformly** (e.g., mm → inch conversion error): Detect via volume ratio being a cube of a common factor (25.4³). Warn the user about possible unit mismatch.

**Assembly import where part order changed**: Match parts by name and/or geometric similarity rather than by index.

**2D import where layers changed**: Match DXF layers by name. New layers flagged as potential new features.

## Implementation Phase

Part update is a Phase 3-4 feature:
- Phase 3: Basic "replace geometry" with diff report and stock check (no ICP, no operation audit)
- Phase 4: Full registration, operation audit, auto-adjustments
- Phase 5: Automatic update from Onshape/FreeCAD with change detection polling
