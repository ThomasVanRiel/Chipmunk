# Decisions Index

Quick reference of all design decisions made during planning sessions. For full rationale, see the linked discussion docs and design docs.

---

## Architecture

| Decision | Detail | Source |
|----------|--------|--------|
| Backend language | Rust (axum) | `docs/00-overview.md` |
| Post-processors | Lua 5.4 via mlua (~300KB embedded VM). Built-ins via `include_str!()`. User post-processors from config directory. | `docs/03-nc-and-postprocessors.md` |
| Frontend | Deferred indefinitely. See `tasks/backlog.md`. | `docs/05-frontend-design.md` |
| Geometry kernel | OpenCascade via `opencascade-rs` v0.2.0 (cxx.rs FFI). Hard dependency. | `discussions/opencascade-bindings.md` |
| Primary geometry format | SVG/DXF → color-grouped entities (Circle, ClosedPath, OpenPath). B-rep used for exact curves internally. | `discussions/brep-geometry.md` |
| Project scope | 2.5D only (SVG/DXF input, top-down view). 3D projects (STEP/STL, Three.js viewport) deferred to backlog. | `tasks/backlog.md` |
| Implementation order | CLI first. Phase 1=scaffolding+import, Phase 2=manual drill, Phase 3=auto drill cycles, Phase 4=2.5D milling. Frontend and 3D are backlog. | `docs/07-implementation-phases.md` |
| Shape persistence | Separate `.brep` file per part alongside `.camproj` JSON | `discussions/brep-geometry.md` #7 |

## Data Model

| Decision | Detail | Source |
|----------|--------|--------|
| FaceInfo granularity | Minimal: id, surface type, normal, area. Richer data computed on demand. | `discussions/brep-geometry.md` #1 |
| STL import strategy | Degraded but functional — sew into B-rep shell, accept triangular faces. Nudge users toward STEP. | `discussions/brep-geometry.md` #2 |
| Face IDs in mesh | Always included (one u32 per triangle). Enables face selection without a second request. | `discussions/brep-geometry.md` #5 |
| Tool identification | Tools have `tool_number` (T1, T2) AND `name`. Post-processor decides which to use for tool calls. | `discussions/api.md` #2.2, `docs/01-data-model.md` |
| Tool number uniqueness | NOT unique within a project — uniqueness enforced per `machine` value only. Supports multi-machine workflows. | `docs/01-data-model.md` |
| Tool library | Global (persistent across projects). Tools copied into project, editable per-project. Import/export via JSON. | `discussions/api.md` #1.8 |
| Setup grouping | Operations grouped under setups. Setup defines WCS, stock, clearance height. Operations inherit with per-operation override. | `discussions/api.md` #2.6, `docs/02-api-design.md` |
| Clearance height | Single height per setup. Full retraction between setups handled by post-processor. | `discussions/api.md` #2.3 |
| Stock | Optional, per-setup. Operator's responsibility. Only needed for simulation/optimization. | `docs/00-overview.md` |
| Entry strategies | Pockets: plunge/helix/ramp (`pocket_entry` field). Profiles: lead-in arc radius. | `discussions/api.md` #2.4 |
| Cutter compensation | CAM mode (software offset) or Controller mode (G41/G42, RL/RR). Per-operation choice. | `docs/04-toolpath-algorithms.md` |
| Arc preservation | Hybrid — profiles in controller mode preserve exact arcs from B-rep. Pockets use polygon offset (tessellated) with arc fitting on output. | `discussions/brep-geometry.md` #4 |
| Auto-persistence | Every change saved immediately. No save button. Undo/redo via JSON patches. | `docs/00-overview.md` |

## Dependency Cleanup

| Decision | Detail | Source |
|----------|--------|--------|
| Drop `stl_io` | OpenCascade handles STL reading | `discussions/brep-geometry.md` #6 |
| Drop `parry3d` | OpenCascade handles sectioning and mesh ops | `discussions/brep-geometry.md` #6 |
| Drop `dxf-rs`, `usvg` from Cargo.toml | Replaced by OpenCascade for geometry construction. Still need Rust crates for *parsing* DXF/SVG, then build OCCT wires from parsed entities. | `discussions/opencascade-bindings.md` |

## Rejected Features

| Feature | Reason | Source |
|---------|--------|--------|
| Machine profiles | Too complex for target audience. Quick NC code generation is the priority. | `discussions/api.md` #2.1 |
| Material library | User problem. Tool data is material-specific; user overrides per-operation. | `discussions/api.md` #2.5 |
| Rest machining | User can do it manually. Not worth the complexity. | `discussions/api.md` #2.9 |
| Probing / measurement cycles | Out of scope. | `discussions/api.md` #2.10 |

## Deferred

| Item | Status | Source |
|------|--------|--------|
| API endpoint response examples | Fill in during implementation | `discussions/deferred-ideas.md` |
| Face/feature selection details | Partially resolved by B-rep face_ids. Tangent face grouping still open. | `discussions/deferred-ideas.md` |
| Setup sheets / job documentation | Revisit after Phase 4 (2.5D milling complete) | `discussions/deferred-ideas.md` |
| Simulation API contracts | Defined in `02-api-design.md` as future placeholders (deferred) | `discussions/api.md` #2.8 |
| Frontend | Browser UI, Three.js viewport, toolpath visualization | `tasks/backlog.md` |
| 3D projects | STEP/STL import, 3D viewport, B-rep slicer | `tasks/backlog.md` |
