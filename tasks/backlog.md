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

Most of the value of a frontend is **geometry selection** (clicking which contour to use) and **toolpath visualization** (verifying before cutting). Both are solved differently by the SVG color workflow — the colors in Inkscape serve as the selection mechanism, and `--dry-run` gives a text summary. The frontend is still useful but not blocking.

---

## Inkscape Extension

Appears under **Extensions > CAM** in Inkscape. Eliminates the file-management step of the CLI workflow — the user draws in Inkscape and generates NC without leaving the application.

### How Inkscape extensions work

- Two files per extension: `camproject.inx` (XML descriptor) and `camproject.py` (Python handler)
- Installed to `~/.config/inkscape/extensions/` (user) or system-wide
- Inkscape passes the current SVG to the Python script; the script can show dialogs, read/write files, call external programs

### Implementation options

**Option A — Shell out to CLI** (simplest)
- Extension dialog collects job params (or points to a `job.yaml`)
- Extension writes a temp YAML, calls `camproject mill <temp.svg> --params <temp.yaml>`
- Shows output path in a result dialog

**Option B — Direct Python bindings** (tighter integration)
- If camproject exposes a Python API (via PyO3), the extension calls it directly
- Avoids temp files; shows progress in Inkscape's status bar

**Option C — Print/plot driver** (as Inkscape's print function)
- Register camproject as a system "printer"
- User does File > Print → selects "CAMproject" printer → NC file written
- Inkscape passes PostScript/PDF; the driver converts to NC
- Complex to set up (system-level driver); less control than options A/B
- Worth investigating once the extension approach is working

### Tasks (when scheduled)

- [ ] `inkscape-extension/camproject.inx` — XML descriptor (menu location, parameter inputs)
- [ ] `inkscape-extension/camproject.py` — reads SVG from stdin, calls CLI or library, writes NC
- [ ] Dialog: job YAML path or inline params (tool number, diameter, postprocessor, output dir)
- [ ] Install script / packaging

---

## 3D Projects (STEP/STL Input)

Most milling is 2.5D — the toolpath pipeline is identical regardless of whether the input is a DXF contour or a B-rep section. 3D support adds:

- **STEP/STL import** via OpenCascade (`STEPControl_Reader`, `BRepBuilderAPI_Sewing`)
- **B-rep slicer** (`toolpath/slicer.rs`): `BRepAlgoAPI_Section(shape, plane_at_z)` → `geo::MultiPolygon`; same polygon offset pipeline used from there
- **Face orientation**: click a face to set Z-up; backend reads face normal from B-rep
- **CLI**: `camproject mill part.step --slice-z 0 --params job.yaml` — slice at Z, then same color/op pipeline but with DXF-less geometry selection (face IDs or auto-detect all planar faces)
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

For when the source drawing changes and existing operations need to be re-validated. See `docs/09-part-update.md`.

- Geometry diff: detect added/removed/modified contours
- ICP registration (Besl & McKay 1992): realign if origin shifted
- Operation audit: flag operations whose referenced geometry changed
- User review: accept/reject per operation

---

## Stock Simulation

- Z-buffer material removal (dexel or height-map)
- CLI: `camproject simulate part.svg --params job.yaml` → renders a 2D material-remaining map (SVG or PNG output)
- No full 3D simulation needed for 2.5D work — a top-down depth map is sufficient

---

## CAD Integrations

- **Watch folder**: `integrations/watch_folder.rs` — rerun job when SVG changes on disk
- **Onshape**: export DWG/SVG from Onshape part studio via REST API, feed into mill workflow
- **FreeCAD**: export DXF from `.FCStd` via CLI bridge
