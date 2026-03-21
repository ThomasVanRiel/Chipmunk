# Backlog

Deferred items with clear design but no scheduled phase. Implement after the core CLI workflow (Phases 1–4) is solid and validated on real hardware.

---

## Browser Frontend

Full web UI for users who prefer not to use the CLI or Inkscape.

- 2D canvas viewport: render SVG/DXF geometry, toolpath overlay, pan/zoom, grid
- Operations panel: add/edit operations visually, click geometry to select contours and points
- NC preview panel: syntax-highlighted output, post-processor dropdown
- Export dialog: single file vs by-tool, download ZIP
- WebSocket progress bar during toolpath generation
- Undo/redo

Most of the value of a frontend is **geometry selection** (clicking which contour to use) and **toolpath visualization** (verifying before cutting). Both are solved differently by the SVG color workflow — the colors in Inkscape serve as the selection mechanism, and `--check` gives a text summary. The frontend is still useful but not blocking.

---

## Inkscape Extension

Appears under **Extensions > CAM** in Inkscape. Eliminates the file-management step of the CLI workflow — the user draws in Inkscape and generates NC without leaving the application.

### How Inkscape extensions work

- Two files per extension: `chipmunk.inx` (XML descriptor) and `chipmunk.py` (Python handler)
- Installed to `~/.config/inkscape/extensions/` (user) or system-wide
- Inkscape passes the current SVG to the Python script; the script can show dialogs, read/write files, call external programs

### Implementation options

**Option A — Shell out to CLI** (simplest)
- Extension dialog collects job params (or points to a `job.yaml`)
- Extension writes a temp YAML (with `geometry:` pointing to the SVG), calls `chipmunk <temp.yaml>`
- Shows output path in a result dialog

**Option B — Direct Python bindings** (tighter integration)
- If chipmunk exposes a Python API (via PyO3), the extension calls it directly
- Avoids temp files; shows progress in Inkscape's status bar

**Option C — Print/plot driver** (as Inkscape's print function)
- Register chipmunk as a system "printer"
- User does File > Print → selects "Chipmunk" printer → NC file written
- Inkscape passes PostScript/PDF; the driver converts to NC
- Complex to set up (system-level driver); less control than options A/B
- Worth investigating once the extension approach is working

### Tasks (when scheduled)

- [ ] `inkscape-extension/chipmunk.inx` — XML descriptor (menu location, parameter inputs)
- [ ] `inkscape-extension/chipmunk.py` — reads SVG from stdin, calls CLI or library, writes NC
- [ ] Dialog: job YAML path or inline params (tool number, diameter, postprocessor, output dir)
- [ ] Install script / packaging

---

## 3D Projects (STEP/STL Input)

Most milling is 2.5D — the toolpath pipeline is identical regardless of whether the input is a DXF contour or a B-rep section. 3D support adds:

- **STEP/STL import** via OpenCascade (`STEPControl_Reader`, `BRepBuilderAPI_Sewing`)
- **B-rep slicer** (`toolpath/slicer.rs`): `BRepAlgoAPI_Section(shape, plane_at_z)` → `geo::MultiPolygon`; same polygon offset pipeline used from there
- **Face orientation**: click a face to set Z-up; backend reads face normal from B-rep
- **CLI**: `chipmunk job.yaml --geometry part.step --slice-z 0` — slice at Z, then same color/op pipeline but with DXF-less geometry selection (face IDs or auto-detect all planar faces)
- **Three.js viewport** (frontend): tessellated mesh + `face_ids` + `TessellatedEdge` list; orbit camera; face/edge pick

Note on rotary axis: a rotary 4th axis keeps the workflow 2.5D (each angular position is a 2.5D setup). The user handles WCS selection manually — no special support needed.

---

## Sinumerik Post-Processor

- `postprocessors/sinumerik.lua`:
  - `CYCLE81` (simple drill), `CYCLE83` (peck), `CYCLE84` (tap)
  - `/1`–`/8` block delete for optional operations
  - Cutter comp: `G41`/`G42` with `D` offset register
  - Tool call: `T1 D1 M6`

---

## Part Update Pipeline

For when the source drawing changes and existing operations need to be re-validated. See `design/docs/deferred/09-part-update.md`.

- Geometry diff: detect added/removed/modified contours
- ICP registration (Besl & McKay 1992): realign if origin shifted
- Operation audit: flag operations whose referenced geometry changed
- User review: accept/reject per operation

---

## Stock Simulation

- Z-buffer material removal (dexel or height-map)
- CLI: `chipmunk simulate job.yaml` → renders a 2D material-remaining map (SVG or PNG output)
- No full 3D simulation needed for 2.5D work — a top-down depth map is sufficient

---

## REST API

Axum HTTP server exposing the same core library functions used by the CLI. Required before any frontend work. The CLI and API are peers — neither wraps the other.

Key design: `chipmunk-server` is a separate binary (built with `--features server`). All endpoints call library functions directly, no HTTP to self. The CLI binary has no dependency on axum or tokio.

Endpoints defined in `design/docs/deferred/02-api-design.md`. Implement when a frontend or remote access is needed.

Tasks (when scheduled):
- [ ] Add axum, tokio, tower, tower-http to `Cargo.toml`
- [ ] `src/api/mod.rs` — router setup, AppState
- [ ] `src/api/routes.rs` — endpoint handlers (thin wrappers over library functions)
- [ ] `GET /api/health`, `GET /api/postprocessors`
- [ ] Project CRUD: `GET/POST /api/project`, parts upload, export endpoints
- [ ] Tools, setups, operations CRUD
- [ ] `src/bin/chipmunk_server.rs` — server entry point with `--dev` and `--port` flags

---

## `chipmunk wizard`

Interactive CLI subcommand that guides the user step by step — operation type, coordinates, tool parameters, post-processor, output file. For quick jobs without a drawing or YAML file.

- Prompts in sequence, shows defaults, allows free-form input
- At the end, optionally writes a YAML file so the session is reproducible
- Implement after the core CLI workflow is solid and the YAML format is stable

---

## Turning

Lathe toolpaths are structurally different from milling — the part rotates, the tool moves in XZ. The post-processor architecture handles this cleanly (a turning post-processor is just another Lua module), but the toolpath generators and NC IR need turning-specific additions:

- **Turning operations**: facing, OD/ID profiling, grooving, threading, parting
- **Turning cycles**: G71/G72 roughing cycles (Fanuc/Haas), `CYCLE95` (Sinumerik), `TURNING` (Heidenhain)
- **Coordinate system**: X is diameter (or radius depending on controller), Z is along the spindle axis
- **Tool nose radius compensation**: equivalent to cutter compensation for mills
- **Live tooling**: optional — cross-drilling, milling on a turning centre

The Haas example post-processor is a natural starting point since the Haas TL-series lathes use near-standard G-code.

---

## Wire EDM

Wire EDM is structurally different from milling. The machine handles its own CAM (cut conditions, wire speed, flushing, multi-pass finishing) — Chipmunk's job is to export the **contour geometry as G-code**, which the machine imports and uses as the basis for its own programming.

### Output format

Standard-ish G-code contour: `G00`/`G01`/`G02`/`G03` moves in XY. No tool compensation, no cutting parameters — those live on the machine. Exact arcs from the B-rep should be preserved as `G02`/`G03` rather than linearised, since wire EDM tolerances are tight and arc approximation introduces error.

### Tapered cuts (4-axis)

4-axis wire EDM moves the upper and lower wire guides independently (XY for one guide, UV for the other), allowing tapered walls or transition cuts where the top and bottom profiles differ. Two modes:

- **Constant taper** — single contour + taper angle. The machine offsets the UV path automatically. Simple to program; Chipmunk emits the XY contour and a taper angle parameter.
- **Two-profile** — upper and lower contours differ (e.g. square at bottom, circle at top). Output is `XYUV` G-code: XY drives the lower guide, UV drives the upper guide simultaneously. Requires two separate contour paths in the YAML, one per guide. The machine interpolates between them. More complex; needs a way to associate the two contours in the job file.

Both modes are worth supporting. The SVG color convention maps cleanly: one color = lower contour, a second paired color = upper contour (or the same color with a `taper_angle` field for the simple case).

### Entry holes

Internal features (pockets, slots) require the wire to be threaded through a pre-drilled entry hole before the cut starts. The G-code contour must begin at the entry hole position, not on the contour itself. The YAML should be able to specify entry hole position per contour (or let the operator mark it in the SVG as a separate point geometry).

### Tasks (when scheduled)

- [ ] Wire EDM operation type: `type: wedm_contour`
- [ ] `taper_angle` field (constant taper, single contour)
- [ ] `upper_color` / `lower_color` pairing for two-profile cuts
- [ ] Entry point specification (SVG point marker or explicit XY in YAML)
- [ ] Post-processor: contour-only G-code output (no spindle, no Z moves, XY + optional UV)
- [ ] Arc preservation: ensure `G02`/`G03` pass through to output without linearisation

---

## CAD Integrations

- **Watch folder**: `integrations/watch_folder.rs` — rerun job when SVG changes on disk
- **Onshape**: export DWG/SVG from Onshape part studio via REST API, feed into mill workflow
- **FreeCAD**: export DXF from `.FCStd` via CLI bridge
