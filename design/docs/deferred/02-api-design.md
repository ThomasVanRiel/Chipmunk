# API Design

> **DEFERRED** — The REST API is not being built in Phases 1–4. The CLI is the primary interface. The API will be implemented when a frontend or remote access is needed. See `tasks/backlog.md`. This document defines the intended design and serves as a contract for future implementation.

## Overview

The backend (Rust/axum) exposes a REST API for CRUD operations and file transfers, plus a WebSocket for real-time progress updates during toolpath generation.

All endpoints are prefixed with `/api/`.

**Single-project model**: The server manages exactly one active project at a time. State is held in a shared `Arc<RwLock<Project>>`. Creating or loading a project replaces the current one (the previous project is auto-saved first). This keeps the API simple — there is no project ID in URLs. Multi-project support (project registry, `/api/projects/{id}/...` routing) is a future enhancement if needed.

## REST Endpoints

### Project

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/project` | Create a new project (replaces current) |
| `GET` | `/api/project` | Get current project state |
| `PUT` | `/api/project` | Update project settings (name, units) |
| `POST` | `/api/project/load` | Load project from .camproj file (replaces current) |
| `GET` | `/api/project/download` | Download project as .camproj file |
| `POST` | `/api/project/undo` | Undo last action |
| `POST` | `/api/project/redo` | Redo last undone action |
| `GET` | `/api/project/history` | Get command history (undo/redo stack) |
| `DELETE` | `/api/project/history` | Clear undo/redo history |

Note: There is no explicit "save" endpoint — the project is auto-persisted on every mutation. Creating or loading a project auto-saves the current project first, then replaces it. The server always has exactly one active project.

#### `POST /api/project`
```json
// Request
{ "name": "My Part", "units": "mm" }

// Response 201
{ "name": "My Part", "units": "mm", "parts": [], "tools": [], "operations": [], "setups": [] }
```

#### `GET /api/project/download`
Returns the `.camproj` file as a download for backup or sharing.
```
Content-Type: application/octet-stream
Content-Disposition: attachment; filename="My Part.camproj"
```

#### `GET /api/project/history`
Returns the command history and cursor position. The frontend uses this to populate undo/redo button tooltips and the history panel.
```json
// Response 200
{
  "cursor": 3,
  "commands": [
    { "id": "...", "timestamp": "2026-03-20T10:00:00Z", "command_type": "add_operation", "description": "Added facing operation" },
    { "id": "...", "timestamp": "2026-03-20T10:01:00Z", "command_type": "update_operation", "description": "Changed depth to -5mm" },
    { "id": "...", "timestamp": "2026-03-20T10:02:00Z", "command_type": "add_tool", "description": "Added 6mm end mill" }
  ]
}
```

The `cursor` indicates the current position. Commands before `cursor` can be undone; commands at or after `cursor` can be redone. The response omits the full JSON patches to keep it lightweight — only metadata is returned.

### Global Tool Library

Persistent across projects. Tools are copied into a project when selected.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/tools` | List all tools in global library |
| `POST` | `/api/tools` | Add a tool to global library |
| `PUT` | `/api/tools/{id}` | Update a global library tool |
| `DELETE` | `/api/tools/{id}` | Remove from global library |
| `GET` | `/api/tools/export` | Download tool library as JSON file |
| `POST` | `/api/tools/import` | Upload tool library JSON (merge into existing) |

### Parts (Geometry Import)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/project/parts` | Upload and import a geometry file |
| `GET` | `/api/project/parts` | List all imported parts |
| `GET` | `/api/project/parts/{id}` | Get part metadata |
| `GET` | `/api/project/parts/{id}/mesh` | Get triangulated mesh for Three.js |
| `GET` | `/api/project/parts/{id}/contour?z={z}` | Get 2D contour at Z height |
| `DELETE` | `/api/project/parts/{id}` | Remove a part |
| `PUT` | `/api/project/parts/{id}/transform` | Update part position/rotation |
| `POST` | `/api/project/parts/{id}/update` | Upload new geometry and preview changes (does not apply) |
| `POST` | `/api/project/parts/{id}/update/apply` | Apply a previewed geometry update |

#### `POST /api/project/parts`
Multipart file upload. Accepts `.stl`, `.dxf`, `.svg`, `.step` files.
```
Content-Type: multipart/form-data
file: <binary>
```
```json
// Response 201
{
  "id": "550e8400-...",
  "name": "bracket.stl",
  "source_format": "stl",
  "bounding_box": {
    "min_x": 0, "min_y": 0, "min_z": 0,
    "max_x": 50, "max_y": 30, "max_z": 15
  }
}
```

#### `GET /api/project/parts/{id}/mesh`
Returns the full selection mesh for Three.js: triangle data for face rendering/picking, plus edge polylines for edge rendering/picking.

```json
// Response 200 (Accept: application/json)
{
  "vertices": [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, ...],
  "normals": [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, ...],
  "indices": [0, 1, 2, ...],
  "face_ids": [0, 0, 0, 1, 1, 2, ...],
  "edges": [
    { "edge_id": 0, "points": [0.0, 0.0, 0.0, 10.0, 0.0, 0.0] },
    { "edge_id": 1, "points": [10.0, 0.0, 0.0, 10.0, 5.0, 0.0, 9.5, 5.3, 0.0, ...] },
    ...
  ]
}
```

All arrays are flat (`[x0,y0,z0, x1,y1,z1, ...]`), matching Three.js `BufferAttribute` layout directly. `face_ids` has one entry per triangle (length = `indices.length / 3`). Each edge's `points` is a tessellated polyline — straight edges have two points, arcs and splines have more. Edge tessellation uses the same deflection parameter as the triangle mesh so visual density is consistent.

**Face selection**: raycast against the triangle mesh → triangle index → `face_ids[triangleIndex]` → B-rep face.

**Edge selection**: Three.js `Raycaster` with `linePrecision` set to ~0.5mm (world units, scales with zoom) raycasts against a `LineSegments` object per edge → `edge_id`. The frontend builds one `LineSegments` per edge (or uses a multi-segment approach with per-point `edge_id` attributes) so a hit can be mapped back to the correct `edge_id`.

**Binary format** (future optimization): For large meshes, the client can request `Accept: application/octet-stream` to receive a packed binary layout instead of JSON. Not needed for v1 — JSON is sufficient for typical part sizes.

#### `POST /api/project/parts/{id}/update`
Upload new geometry to preview changes before applying. The update is NOT applied — the response is a change report for the user to review. See `09-part-update.md` for the full pipeline.

```
Content-Type: multipart/form-data
file: <binary>
```
Alternatively, refresh from the original source:
```json
// Request
{ "refresh_from_source": true }
```
```json
// Response 200
{
  "diff": {
    "old_bbox": { "min_x": 0, "max_x": 100, ... },
    "new_bbox": { "min_x": 0, "max_x": 105, ... },
    "change_type": "dimensions_only"
  },
  "registration": {
    "transform": [[1,0,0,0], ...],
    "method_used": "icp",
    "confidence": 0.95
  },
  "operation_audits": [
    {
      "operation_id": "...",
      "status": "ok",
      "issues": [],
      "auto_adjustments": []
    },
    {
      "operation_id": "...",
      "status": "adjusted",
      "issues": ["Depth may need scaling"],
      "auto_adjustments": ["Depth scaled 20mm → 22mm"]
    }
  ]
}
```

#### `POST /api/project/parts/{id}/update/apply`
Apply a previously previewed geometry update.

```json
// Request
{
  "accept_adjustments": true,
  "update_stock": true,
  "regenerate_toolpaths": true
}

// Response 200 — updated project state
{ "name": "My Part", "units": "mm", "parts": [...], ... }
```

### Tools

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/project/tools` | List all tools |
| `POST` | `/api/project/tools` | Add a new tool |
| `PUT` | `/api/project/tools/{id}` | Update a tool |
| `DELETE` | `/api/project/tools/{id}` | Remove a tool |

#### `POST /api/project/tools`
```json
// Request
{
  "name": "6mm 2-flute end mill",
  "tool_number": 1,
  "machine": "Haas VF-2",
  "type": "end_mill",
  "diameter": 6.0,
  "flute_length": 20.0,
  "total_length": 50.0,
  "num_flutes": 2,
  "recommended_feed_rate": 800.0,
  "recommended_plunge_rate": 300.0,
  "recommended_spindle_speed": 18000.0,
  "recommended_depth_per_pass": 2.0,
  "recommended_stepover": 0.4,
  "recommended_coolant": "flood"
}

// Response 201
{ "id": "...", ...same fields... }
```

Tools carry geometry + recommended cutting data. When a tool is selected for an operation, the recommended values pre-fill the operation's parameters. The user can override any value per-operation.

**Tool identification**: Each tool has a `tool_number` (integer, e.g., 1, 2, 3), a `name` (string, e.g., "6MM_EM"), and an optional `machine` (string, e.g., "Haas VF-2", "Manual"). The post-processor decides which identifier to use in the NC output:
- G-code controllers (Fanuc, LinuxCNC, Grbl): `T1 M6` — uses `tool_number`
- Heidenhain TNC: `TOOL CALL 1 Z S18000` or `TOOL CALL "6MM_EM" Z S18000` — can use either; the post-processor has a `tool_call_by_name: bool` option

**Tool numbers are NOT required to be unique** within a project. A project may target multiple machines, each with its own tool carousel. The same tool number can appear on different machines (e.g., T1 on the Haas is a 6mm end mill, T1 on the Tormach is a 3mm ball nose). The `machine` field distinguishes them. Uniqueness is only enforced within the same `machine` value.

The `machine` field is a free-form string — there is no machine profile entity. It's purely a label for organizing tools when a project spans multiple machines.

#### `GET /api/tools/export`
Downloads the global tool library as a JSON file for sharing or backup.
```
Content-Type: application/octet-stream
Content-Disposition: attachment; filename="tool_library.json"
```

#### `POST /api/tools/import`
Uploads a tool library JSON file. Tools are merged into the existing library — duplicates (matched by name + type + diameter) are skipped, new tools are added. Returns the number of tools added.
```json
// Response 200
{ "added": 5, "skipped": 2, "total": 12 }
```

### Setups

A setup groups operations that share the same workholding — same WCS, stock, and clearance height. Operations inherit these from their parent setup but can override any value. NC export is typically per-setup (one program per fixture).

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/project/setups` | List all setups (ordered) |
| `POST` | `/api/project/setups` | Create a new setup |
| `GET` | `/api/project/setups/{id}` | Get setup details |
| `PUT` | `/api/project/setups/{id}` | Update setup parameters |
| `DELETE` | `/api/project/setups/{id}` | Delete setup (and its operations) |
| `PUT` | `/api/project/setups/reorder` | Change setup order |

#### `POST /api/project/setups`
```json
// Request
{
  "name": "Setup 1 — Top",
  "wcs": {
    "origin": [0, 0, 0],
    "rotation": [0, 0, 0],
    "work_offset": "G54"
  },
  "stock": {
    "shape": "box",
    "width": 110.0,
    "height": 85.0,
    "depth": 25.0
  },
  "clearance_height": 50.0
}

// Response 201
{ "id": "setup-uuid-1", ...same fields..., "operations": [] }
```

**Clearance height**: Defines the safe Z for rapids between operations within this setup. The post-processor inserts a full retraction to clearance height between setups when generating multi-setup programs. This is the only height the user configures — retract between passes within an operation is handled by the toolpath generator (typically stock top + a small margin).

**WCS inheritance**: Operations within a setup inherit its WCS, stock, and clearance height. An operation can override any inherited value by setting it explicitly. If the setup's WCS changes, all operations that haven't overridden it update automatically.

### Operations

Operations always belong to a setup. The `setup_id` field is required when creating an operation.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/project/operations` | List all operations across all setups (ordered) |
| `GET` | `/api/project/operations/{id}` | Get a single operation |
| `POST` | `/api/project/operations` | Add a new operation |
| `PUT` | `/api/project/operations/{id}` | Update operation parameters |
| `DELETE` | `/api/project/operations/{id}` | Remove an operation |
| `PUT` | `/api/project/operations/reorder` | Change operation execution order |
| `POST` | `/api/project/operations/{id}/generate` | Generate toolpath for this operation |
| `POST` | `/api/project/operations/generate-all` | Generate all toolpaths |
| `POST` | `/api/project/operations/{id}/duplicate` | Duplicate operation (copy all params) |

#### `POST /api/project/operations`
```json
// Request
{
  "name": "Rough pocket",
  "type": "pocket",
  "setup_id": "setup-uuid-1",
  "geometry_id": "550e8400-...",
  "tool_id": "660e8400-...",
  "feed_rate": 800.0,
  "plunge_rate": 300.0,
  "spindle_speed": 18000.0,
  "depth_per_pass": 2.0,
  "start_depth": 0.0,
  "final_depth": -10.0,
  "pocket_stepover": 0.4,
  "pocket_strategy": "contour_parallel",
  "pocket_entry": "helix"
}

// Response 201
{ "id": "...", "setup_id": "setup-uuid-1", ...same fields..., "toolpath": null }
```

Operations inherit WCS, stock, and clearance height from their setup. To override, include the field explicitly in the request (e.g., add `"wcs": {...}` to use a different WCS than the setup default).

**Entry strategy** (type-specific):
- **Pocket operations**: `pocket_entry` field — `"plunge"` (default), `"helix"`, or `"ramp"`. Controls how the tool enters the material at the start of each pocket pass.
- **Profile operations**: `lead_in_radius` field (existing) — arc tangent entry. Set to `null` or `0` for direct plunge.

#### `PUT /api/project/operations/reorder`
Accepts the full ordered list of operation IDs. The server validates that the list contains exactly the current set of operation IDs (no additions or removals), then reorders to match.

```json
// Request
{
  "order": ["op-uuid-3", "op-uuid-1", "op-uuid-2"]
}

// Response 200
{ "order": ["op-uuid-3", "op-uuid-1", "op-uuid-2"] }
```

Returns `400` if the ID list doesn't match the current operation set.

#### `POST /api/project/operations/{id}/generate`
Starts toolpath generation. Returns immediately with a job ID. Progress is reported via WebSocket.

```json
// Response 202
{ "job_id": "job-abc123", "status": "started" }
```

### Toolpath Data

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/project/operations/{id}/toolpath` | Get toolpath segments for visualization |
| `GET` | `/api/project/operations/{id}/toolpath/stats` | Get toolpath statistics |

#### `GET /api/project/operations/{id}/toolpath`

Each segment specifies a destination point. The `start_position` field gives the tool position before the first segment (typically the machine home or safe position), so the frontend can draw the complete path including the first move.

```json
// Response 200
{
  "operation_id": "...",
  "start_position": { "x": 0, "y": 0, "z": 25 },
  "segments": [
    { "type": "rapid", "x": 0, "y": 0, "z": 5 },
    { "type": "rapid", "x": 10, "y": 10, "z": 5 },
    { "type": "linear", "x": 10, "y": 10, "z": -2, "feed": 300 },
    { "type": "linear", "x": 50, "y": 10, "z": -2, "feed": 800 },
    { "type": "arc_cw", "x": 50, "y": 20, "z": -2, "i": 0, "j": 5, "feed": 800 }
  ],
  "stats": {
    "total_distance_mm": 1234.5,
    "cutting_distance_mm": 987.6,
    "estimated_time_min": 12.3,
    "num_segments": 4567
  }
}
```

### NC Code Export

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/postprocessors` | List available post-processors |
| `POST` | `/api/project/export` | Generate and download NC code |
| `POST` | `/api/project/export/preview` | Preview NC code as text |

#### `GET /api/postprocessors`
```json
// Response 200
[
  { "id": "heidenhain", "name": "Heidenhain TNC", "file_extension": ".h" },
  { "id": "haas",       "name": "Haas",            "file_extension": ".nc" }
]
```

Built-in post-processors are Heidenhain TNC (primary) and Haas (G-code example). User post-processors placed in the config directory appear in this list automatically.

#### `POST /api/project/export/preview`
```json
// Request
{
  "postprocessor": "linuxcnc",
  "setup_id": "setup-uuid-1",        // export one setup (null for all setups)
  "operation_ids": ["op1", "op2"],    // filter within setup (null for all operations in scope)
  "parameterized_feeds": false        // true = feed rates as variables at top of program
}

// Response 200
{
  "postprocessor": "linuxcnc",
  "nc_code": "%\nO0001\nG90 G94 G17\nG21\n...\nM30\n%",
  "line_count": 1234,
  "warnings": []
}
```

#### `POST /api/project/export`
Same request as preview, but returns the file as a download.
```
Content-Type: application/octet-stream
Content-Disposition: attachment; filename="part.ngc"
```

**Single-operation shortcut**: `POST /api/project/operations/{id}/export` and `.../export/preview` are convenience aliases that set `operation_ids` to `[id]`. Same request/response format otherwise (postprocessor selection, parameterized_feeds).

## WebSocket

### Connection

```
ws://localhost:8000/api/ws
```

The WebSocket is bidirectional. The server pushes progress and state updates; the client can send control messages (e.g., cancel a running job).

### Messages (Client → Server)

#### Cancel toolpath generation
```json
{
  "type": "cancel_job",
  "job_id": "job-abc123"
}
```

The server acknowledges cancellation with a `toolpath_error` message (error: "cancelled"). If the job already completed or doesn't exist, the cancel is silently ignored.

### Messages (Server → Client)

#### Toolpath generation progress
```json
{
  "type": "toolpath_progress",
  "job_id": "job-abc123",
  "operation_id": "...",
  "progress": 0.45,
  "message": "Generating pocket pass 3/5"
}
```

#### Toolpath generation complete
```json
{
  "type": "toolpath_complete",
  "job_id": "job-abc123",
  "operation_id": "...",
  "stats": { ... }
}
```

#### Toolpath generation error
```json
{
  "type": "toolpath_error",
  "job_id": "job-abc123",
  "operation_id": "...",
  "error": "Tool diameter larger than pocket width"
}
```

#### Project state changed
```json
{
  "type": "project_updated",
  "changed": ["operations", "tools"]
}
```

## Error Handling

All error responses follow this format:

```json
{
  "error": "Short error code",
  "detail": "Human-readable description of what went wrong",
  "field": "optional_field_name",       // for validation errors
  "references": ["optional-uuid", ...]  // for conflict errors — IDs of related resources
}
```

Standard HTTP status codes:
- `400` — Invalid request (bad parameters, missing required fields)
- `404` — Resource not found (unknown part/tool/operation ID)
- `409` — Conflict (resource is referenced by other resources)
- `422` — Validation error (e.g., negative diameter)
- `500` — Internal server error

#### `409` example — deleting a tool used by operations
```json
// DELETE /api/project/tools/{id}
// Response 409
{
  "error": "tool_in_use",
  "detail": "Tool '6mm end mill' is used by 2 operations",
  "references": ["op-uuid-1", "op-uuid-2"]
}
```

The `references` field lists the operation IDs that depend on the tool, so the frontend can show the user which operations would be affected.

## Health & Status

### `GET /api/health`

Returns server status including subsystem readiness. The frontend polls this on startup to know when the backend is fully available.

```json
// Response 200
{
  "status": "ok",
  "version": "0.1.0",
  "postprocessors_loaded": 6,
  "project_loaded": true
}
```

`postprocessors_loaded` is the count of available post-processors (built-in + discovered from config directory). Post-processors are Lua modules embedded at compile time — no runtime initialization delay.

## Future: Simulation (Deferred)

These endpoints are defined as contracts for future implementation. They are **not** part of v1.

### Simulation

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/project/simulate` | Start stock removal simulation (async) |
| `GET` | `/api/project/simulation` | Get simulation result (remaining stock) |
| `POST` | `/api/project/simulate/step` | Simulate up to a specific operation |

#### `POST /api/project/simulate`
Starts a Z-buffer stock removal simulation across all operations (or a specified subset). Returns a job ID; progress reported via WebSocket.

```json
// Request
{
  "setup_id": "setup-uuid-1",
  "up_to_operation_id": "op-uuid-3",  // null = all operations
  "resolution": 0.1                    // Z-buffer resolution in mm
}

// Response 202
{ "job_id": "sim-abc123", "status": "started" }
```

#### `GET /api/project/simulation`
Returns the simulation result as a heightmap or mesh for viewport rendering.

```json
// Response 200
{
  "job_id": "sim-abc123",
  "format": "heightmap",
  "width": 1000,
  "height": 800,
  "origin": [0, 0],
  "resolution": 0.1,
  "data": [25.0, 25.0, 24.5, ...]  // Z heights in row-major order
}
```

WebSocket messages follow the same pattern as toolpath generation: `simulation_progress`, `simulation_complete`, `simulation_error`.

## Future: Integrations (Phase 4-5)

The `/api/integrations/` namespace is reserved for CAD system integrations. See `08-integrations.md` for details.

Planned:
- `/api/integrations/onshape/` — Onshape API (connect, browse documents, import parts, refresh)
- `/api/integrations/freecad/` — FreeCAD CLI bridge (browse, export, import)
- `/api/integrations/watch/` — Watch folder (configure path, start/stop, auto-import on change)

These endpoints will be specified in detail when implementation begins.

## Static File Serving

> **Deferred** — The frontend is not being built in Phases 1–4. See `tasks/backlog.md`.

When a frontend is built, the axum server would serve it as static files in production:
- `GET /` → `frontend/dist/index.html`
- `GET /assets/*` → `frontend/dist/assets/`

In development, the frontend dev server (Vite on `:5173`) would proxy `/api` requests to the axum backend (`:8000`). For now, `chipmunk-server` exposes the API only.

## Pagination

Pagination is not needed for v1. The expected scale (tens of parts, tens of operations, dozens of tools) does not warrant it. If this changes, endpoints returning lists (`GET .../parts`, `GET .../operations`, `GET /api/tools`) can be extended with `?offset=N&limit=N` query parameters without breaking existing clients (default: return all).
