# API Design

## Overview

The backend exposes a REST API for CRUD operations and file transfers, plus a WebSocket for real-time progress updates during toolpath generation.

All endpoints are prefixed with `/api/`.

## REST Endpoints

### Project

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/project` | Create a new project |
| `GET` | `/api/project` | Get current project state |
| `PUT` | `/api/project` | Update project settings (name, units) |
| `POST` | `/api/project/save` | Save project to .camproj file |
| `POST` | `/api/project/load` | Load project from .camproj file |

#### `POST /api/project`
```json
// Request
{ "name": "My Part", "units": "mm" }

// Response 201
{ "name": "My Part", "units": "mm", "stock": null, "parts": [], "tools": [], "operations": [] }
```

### Stock

| Method | Path | Description |
|--------|------|-------------|
| `PUT` | `/api/project/stock` | Define or update stock dimensions |
| `DELETE` | `/api/project/stock` | Remove stock definition |

#### `PUT /api/project/stock`
```json
// Request
{
  "width": 100.0,
  "height": 80.0,
  "depth": 20.0,
  "origin": "top_center"
}

// Response 200
{ "width": 100.0, "height": 80.0, "depth": 20.0, "origin": "top_center" }
```

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
Returns mesh data optimized for Three.js BufferGeometry.
```json
// Response 200
{
  "vertices": [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, ...],
  "normals": [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, ...],
  "indices": [0, 1, 2, ...]
}
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
  "type": "end_mill",
  "diameter": 6.0,
  "flute_length": 20.0,
  "total_length": 50.0,
  "num_flutes": 2,
  "default_feed_rate": 800.0,
  "default_plunge_rate": 300.0,
  "default_spindle_speed": 18000.0,
  "default_depth_per_pass": 2.0,
  "default_stepover": 0.4
}

// Response 201
{ "id": "...", ...same fields... }
```

### Operations

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/project/operations` | List all operations (ordered) |
| `POST` | `/api/project/operations` | Add a new operation |
| `PUT` | `/api/project/operations/{id}` | Update operation parameters |
| `DELETE` | `/api/project/operations/{id}` | Remove an operation |
| `PUT` | `/api/project/operations/reorder` | Change operation execution order |
| `POST` | `/api/project/operations/{id}/generate` | Generate toolpath for this operation |
| `POST` | `/api/project/operations/generate-all` | Generate all toolpaths |

#### `POST /api/project/operations`
```json
// Request
{
  "name": "Rough pocket",
  "type": "pocket",
  "geometry_id": "550e8400-...",
  "tool_id": "660e8400-...",
  "start_depth": 0.0,
  "final_depth": -10.0,
  "depth_per_pass": 2.0,
  "pocket_stepover": 0.4,
  "pocket_strategy": "contour_parallel"
}

// Response 201
{ "id": "...", ...same fields..., "toolpath": null }
```

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
```json
// Response 200
{
  "operation_id": "...",
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
  { "id": "linuxcnc", "name": "LinuxCNC", "file_extension": ".ngc" },
  { "id": "grbl", "name": "Grbl", "file_extension": ".gcode" },
  { "id": "marlin", "name": "Marlin", "file_extension": ".gcode" },
  { "id": "fanuc", "name": "Generic Fanuc", "file_extension": ".nc" }
]
```

#### `POST /api/project/export/preview`
```json
// Request
{
  "postprocessor": "linuxcnc",
  "operation_ids": ["op1", "op2"]  // or null for all
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

## WebSocket

### Connection

```
ws://localhost:8000/api/ws
```

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
  "field": "optional_field_name"  // for validation errors
}
```

Standard HTTP status codes:
- `400` — Invalid request (bad parameters, missing required fields)
- `404` — Resource not found (unknown part/tool/operation ID)
- `409` — Conflict (e.g., deleting a tool that's used by an operation)
- `422` — Validation error (e.g., negative diameter)
- `500` — Internal server error

## Static File Serving

The FastAPI app serves the frontend as static files:
- `GET /` → `frontend/dist/index.html`
- `GET /assets/*` → `frontend/dist/assets/`

In development, the frontend dev server (Vite) runs separately and proxies API calls to FastAPI.
