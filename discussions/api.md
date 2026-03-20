# API Review — Open Items

This document collects all identified gaps in `docs/02-api-design.md`. Each item has a description, rationale, and a **Decision** block for Thomas to fill in.

---

## Part 1: Missing from current design docs

These are things already described in other design docs or implied by the frontend design, but absent from the API spec.

---

### 1.1 Integration endpoints

`08-integrations.md` defines Onshape API routes (lines 160-166) that aren't in the API doc:

```
POST /api/integrations/onshape/connect
GET  /api/integrations/onshape/documents
GET  /api/integrations/onshape/documents/{id}/parts
POST /api/integrations/onshape/import
POST /api/integrations/onshape/refresh/{part_id}
```

FreeCAD and watch folder also need configuration endpoints (path to `freecadcmd`, watch directory path, poll interval).

Phase 4-5, but should be referenced in the API doc so the URL namespace is reserved.

**Decision**:
> Future feature reserve /api/integrations

---

### 1.2 Command history retrieval

The API has `POST .../undo`, `POST .../redo`, and `DELETE .../history`, but no way to **read** the history. The frontend's undo/redo buttons need to show what action will be undone/redone (tooltip: "Undo: Delete facing operation").

Proposed: `GET /api/project/history` returning the command list and cursor position.

**Decision**:
> Add history

---

### 1.3 Project file download

`POST /api/project/load` exists for loading, but there's no endpoint to download the `.camproj` file. Auto-persistence writes server-side, but users need the file for backup, sharing, or moving to another machine.

Proposed: `GET /api/project/download` returning the `.camproj` JSON as a file download.

**Decision**:
> Good idea

---

### 1.4 Endpoint examples missing

These endpoints are listed in the route table but have no request/response examples:

| Endpoint | What's missing |
|----------|---------------|
| `GET /api/project/parts/{id}` | Response format — what metadata? Bounding box, provenance, transform, update history? |
| `GET /api/project/parts/{id}/contour?z={z}` | Response format — GeoJSON? Custom polygon format? Array of coordinate rings? |
| `GET .../operations/{id}/toolpath/stats` | Is this redundant with stats in the toolpath response? If not, what's the response? |
| `POST .../operations/{id}/duplicate` | Does it accept overrides (e.g., name)? Or always a pure copy? |

**Decision**:
> Defer

---

### 1.5 No individual operation GET

`GET /api/project/operations` returns all operations, but there's no `GET /api/project/operations/{id}`. After a mutation, the frontend has to re-fetch the entire list to update one operation.

**Decision**:
> Add operations/id

---

### 1.6 Health / readiness endpoint

No `GET /api/health`. The frontend needs backend liveness detection (especially during startup when PyO3/Python init may be slow). Also useful for the Vite dev proxy.

**Decision**:
> Include full status

---

### 1.7 Face/feature selection for orientation

The frontend design describes click-face for orientation ("set this face as top") and WCS placement. The mesh endpoint returns raw triangles with no face grouping — there's no way to identify which face was clicked and translate that to a transform.

Options:

- **A**: Mesh endpoint includes face group IDs (connected coplanar triangle clusters)
- **B**: Raycasting + normal extraction done client-side, sends normal vector to a `POST .../parts/{id}/orient` endpoint that computes the transform
- **C**: Entirely client-side — compute transform from the clicked face normal in TypeScript, then `PUT .../parts/{id}/transform`

**Decision**:
> We'll need to discuss the data format more in depth. Face grouping is important I think, but it's not as straightforward as finding coplanar faces since tangent faces (e.g. rounds) are also part of a group, but not always. Defer to another discussion

---

### 1.8 Tool library import/export

The global tool library is persistent but has no import/export. Operators commonly share tool libraries between machines or people. Estlcam and Fusion 360 both support tool library files.

Proposed:

- `GET /api/tools/export` — download library as JSON
- `POST /api/tools/import` — upload library JSON (merge or replace)

**Decision**:
> Add both API functiosn

---

## Part 2: Missing compared to other CAM tools

These are features found in Fusion 360 CAM, Mastercam, HSMWorks, Estlcam, and similar tools that aren't covered by the current API at all.

---

### 2.1 Machine profiles

Every serious CAM tool (Fusion 360, Mastercam, HSMWorks) makes you define or select a machine. The current API has no concept of the target machine.

A machine profile would contain:

- Travel limits (X/Y/Z min/max)
- Max spindle speed, max feed rate
- Number of axes (3, 3+2, 5)
- Tool changer type (manual, carousel, rack) and capacity
- Available work offsets
- Supported features (rigid tapping, probing, high-speed modes)
- Default safe Z / clearance height
- Associated default post-processor

This affects:

- **Validation**: don't generate moves outside travel limits
- **Post-processing**: default post-processor selection
- **UI**: don't offer features the machine can't do
- **Tool management**: warn if more tools than changer capacity

Proposed:

- `GET/POST/PUT/DELETE /api/machines` — machine profile CRUD (global, like tool library)
- `PUT /api/project/machine` — assign machine to project
- `GET /api/project/machine` — get assigned machine

**Decision**:
> Machine definition is not necessary. If the work is that critical, a different CAM tool is required. The point of this tool is quick generation of NC Code.

---

### 2.2 Tool numbers / pocket mapping

The Tool struct has a UUID `id` but CNC machines address tools by **number** (T1, T2). This number maps to a physical pocket in the tool changer. The NC code uses it: `T1 M6`, `TOOL CALL 1 Z S18000`.

Without a `tool_number` field, the post-processor can't emit correct tool calls. This is a data model gap that blocks correct NC output.

Mastercam, Fusion 360, and Estlcam all require tool numbers. Typically:

- Tool number is set when adding a tool to a project
- Validation ensures no duplicate numbers within a project
- Post-processor uses the number directly

**Decision**:
> Tool numbers are essential, but tool names might be used for tool calls in NC code

---

### 2.3 Safe Z / retract heights

Every CAM tool has configurable heights for safe movement. Currently completely absent from the Operation struct and the API. The NC compiler doc mentions "rapid to safe Z height" (line 117) but there's no field for it.

Standard heights (Fusion 360 / HSMWorks terminology):

- **Clearance height**: absolute safe Z for rapids between operations or features (e.g., Z+50)
- **Retract height**: Z for retract between passes within a single operation (e.g., stock top + 5mm)
- **Feed height**: Z where the tool transitions from rapid to feed descent (e.g., stock top + 2mm)
- **Top of stock**: reference Z=0 for depth calculations

These should be fields on Operation with project-level defaults. Estlcam keeps it simpler with just "safety height" and "start height" — that might be enough for v1.

**Decision**:
> User problem. We don't care. Clearance height is sufficient.

---

### 2.4 Lead-in / lead-out / linking strategy

The Operation struct has `lead_in_radius` for profiles, but no general entry/exit/linking strategy. Fusion 360 and HSMWorks give detailed control:

**Entry (lead-in)**:

- Arc (tangent entry — the current `lead_in_radius`)
- Line (straight approach at angle)
- Helix (spiral down into pocket — critical for pocket operations)
- Ramp (zigzag descent along cut direction)
- Plunge (straight down — simple but hard on tool)

**Exit (lead-out)**:

- Arc (tangent exit)
- Line
- Retract (straight up)

**Linking between passes**:

- Direct (stay at depth, move to next pass start)
- Retract to retract height (safe but slow)
- Minimum retract (retract just enough to clear stock)
- Ramp between (descend while moving to next pass)

Currently the toolpath generators would have to hardcode these strategies. Making them configurable per-operation lets the user optimize for their material and machine.

**Decision**:
> entry type (plunge/helix/ramp) for pockets, lead-in arc for profiles?

---

### 2.5 Material library

Tools carry recommended cutting data, but feeds/speeds are material-dependent. A 6mm end mill in aluminium runs at 18000 RPM / 800 mm/min; in mild steel it's 4000 RPM / 200 mm/min. Fusion 360, HSMWorks, and Mastercam all have material libraries.

Without a material concept, every operation's feeds/speeds are either manual guesswork or blindly inherited from tool defaults regardless of what's being cut.

Proposed:

- `GET/POST/PUT/DELETE /api/materials` — material CRUD (global library)
- Material has: name, category (metal/wood/plastic), hardness, machinability rating
- Tool recommendations become a lookup: tool + material → feeds/speeds
- Material assigned per-project (or per-operation for multi-material parts)

Estlcam takes a simpler approach: material is just a speed multiplier on tool defaults. That might be enough.

**Decision**:
> Manual entry, material is a user problem. The tool data is specific for a material, the user can use another by overriding.

---

### 2.6 Setup grouping

The data model has per-operation WCS (good for multi-setup), but no **setup** entity grouping operations. In Fusion 360, operations are organized under setups — each setup defines WCS, stock, and orientation; child operations inherit.

Currently with 8 operations across 2 setups, you'd set WCS on all 8 individually. A setup entity would allow:

- Set WCS/stock once per setup, operations inherit
- Reorder operations within a setup
- Export NC per-setup (one program per fixture — very common)
- Visual grouping in the operations panel

Proposed:

- `GET/POST/PUT/DELETE /api/project/setups`
- Operations reference a `setup_id`
- Setup has: name, WCS, stock, machine (optional override)
- Export accepts `setup_id` filter

**Decision**:
> Having shared WCS is quite important for consistency. Of course override is possible always.

---

### 2.7 Setup sheets / job documentation

Operators need a printed reference sheet at the machine. Fusion 360 generates HTML setup sheets. Mastercam has configurable templates. The sheet typically includes:

- Tool list with pocket numbers, diameters, stickout, description
- Operation sequence with estimated times
- WCS location diagram
- Stock dimensions and material
- Total estimated cycle time
- Special notes / instructions

Proposed: `GET /api/project/setup-sheet?format=html` (or `format=pdf`)

**Decision**:
> Defer

---

### 2.8 Simulation / stock verification

Design docs mention simulation for Phase 5 but define no API. Every visual CAM tool has this — showing material removal step by step. Critical for catching collisions and verifying the toolpath actually produces the intended shape.

Proposed:

- `POST /api/project/simulate` — start Z-buffer simulation (async)
- `GET /api/project/simulation` — get remaining stock mesh/heightmap
- WebSocket messages for simulation progress
- Per-operation stepping: simulate up to operation N

**Decision**:
> Define contracts as preparation

---

### 2.9 Rest machining / remaining stock

Common workflow: rough with a 12mm tool, then re-rough or semi-finish with a 6mm tool that only machines where the 12mm couldn't reach. Fusion 360 and Mastercam call this "rest machining" or "remaining stock."

Requires:

- Tracking remaining stock geometry between operations
- `rest_from_operation_id` field on Operation (use previous op's result as stock input)
- Endpoint to query remaining stock model after a given operation

Tightly coupled with simulation (2.8) — rest machining needs the same Z-buffer/stock model.

**Decision**:
> Rest machining is not necessary, user can do it manually

---

### 2.10 Probing / measurement cycles

Some CAM tools (Fusion 360, Mastercam) generate probing routines — the machine probes the part to find the WCS origin or verify dimensions before cutting. Important for multi-setup parts where setup 2's WCS depends on setup 1's result.

Renishaw probing cycles are the most common. Would appear as a new operation type alongside facing/profile/pocket/drill.

**Decision**:
> Don't care

---

## Part 3: Open questions

Space for additional thoughts, concerns, or priorities that don't fit the items above.

> Target audience is hobby and small shop (think prototyping)
