# Decisions Index

Quick reference of all design decisions made during planning sessions. For full rationale, see the linked discussion docs and design docs.

---

## Architecture

| Decision | Detail | Source |
|----------|--------|--------|
| Backend language | Rust (axum) | `design/docs/00-overview.md` |
| Post-processors | Lua 5.4 via mlua (~300KB embedded VM). Built-ins via `include_str!()`. User post-processors from config directory. | `design/docs/03-nc-and-postprocessors.md` |
| Frontend | Deferred indefinitely. See `design/tasks/backlog.md`. | `design/docs/deferred/05-frontend-design.md` |
| Geometry kernel | OpenCascade via `opencascade-rs` v0.2.0 (cxx.rs FFI). Hard dependency. | `design/discussions/opencascade-bindings.md` |
| Primary geometry format | SVG/DXF → color-grouped entities (Circle, ClosedPath, OpenPath). B-rep used for exact curves internally. | `design/discussions/brep-geometry.md` |
| SVG stroke color as operation selector | Each stroke color in the SVG maps to one operation in the YAML. Laser cutters use the same convention (color = cut/engrave/mark layer), so hobbyist users already understand the workflow without explanation. Inkscape is the natural authoring tool. | `design/docs/00-overview.md` |
| Project scope | 2.5D only (SVG/DXF input, top-down view). 3D projects (STEP/STL, Three.js viewport) deferred to backlog. | `design/tasks/backlog.md` |
| Implementation order | CLI first. Phase 1=scaffolding+import, Phase 2=manual drill, Phase 3=auto drill cycles, Phase 4=2.5D milling. Frontend and 3D are backlog. | `design/docs/07-implementation-phases.md` |
| Operation type modeling | Shared `OperationCommon` struct (tool, clearance, name, capabilities) + `OperationVariant` enum + `OperationType` trait. Enum for exhaustive wiring (single match in `kind_impl`). Trait for encapsulated per-type `generate()` and `compile()`. Each operation is self-contained; code duplication across types is preferred over shared abstractions. Drilling-family helpers are fine. | `docs/architecture.md` |
| IO layer owns config-to-operation conversion | `OperationConfig` and `JobConfig` are YAML-specific types in `io/parsing`. They are not part of the core operation type system. Each IO surface (YAML CLI, REST API, bindings) implements its own conversion to `Operation`. There is intentionally no shared "OperationConfig" abstraction in core — the library accepts `Operation` directly. | `src/io/parsing.rs` |
| Shape persistence | Separate `.brep` file per part alongside `.camproj` JSON | `design/discussions/brep-geometry.md` #7 |

## Data Model

| Decision | Detail | Source |
|----------|--------|--------|
| FaceInfo granularity | Minimal: id, surface type, normal, area. Richer data computed on demand. | `design/discussions/brep-geometry.md` #1 |
| STL import strategy | Degraded but functional — sew into B-rep shell, accept triangular faces. Nudge users toward STEP. | `design/discussions/brep-geometry.md` #2 |
| Face IDs in mesh | Always included (one u32 per triangle). Enables face selection without a second request. | `design/discussions/brep-geometry.md` #5 |
| Tool identification | Tools have `tool_number` (T1, T2) AND `name`. Post-processor decides which to use for tool calls. | `design/discussions/api.md` #2.2, `design/docs/01-data-model.md` |
| Tool number uniqueness | NOT unique within a project — uniqueness enforced per `machine` value only. Supports multi-machine workflows. | `design/docs/01-data-model.md` |
| Tool library | Global (persistent across projects). Tools copied into project, editable per-project. Import/export via JSON. | `design/discussions/api.md` #1.8 |
| Setup grouping | Operations grouped under setups. Setup defines WCS, stock, clearance height. Operations inherit with per-operation override. | `design/discussions/api.md` #2.6, `design/docs/deferred/02-api-design.md` |
| Clearance height | Single height per setup. Full retraction between setups handled by post-processor. | `design/discussions/api.md` #2.3 |
| Stock | Optional, per-setup. Operator's responsibility. Only needed for simulation/optimization. | `design/docs/00-overview.md` |
| Entry strategies | Pockets: plunge/helix/ramp (`pocket_entry` field). Profiles: lead-in arc radius. | `design/discussions/api.md` #2.4 |
| Cutter compensation | CAM mode (software offset) or Controller mode (G41/G42, RL/RR). Per-operation choice. | `design/docs/04-toolpath-algorithms.md` |
| Arc preservation | Hybrid — profiles in controller mode preserve exact arcs from B-rep. Pockets use polygon offset (tessellated) with arc fitting on output. | `design/discussions/brep-geometry.md` #4 |
| Auto-persistence | Every change saved immediately. No save button. Undo/redo via JSON patches. | `design/docs/00-overview.md` |

## Dependency Cleanup

| Decision | Detail | Source |
|----------|--------|--------|
| Drop `stl_io` | OpenCascade handles STL reading | `design/discussions/brep-geometry.md` #6 |
| Drop `parry3d` | OpenCascade handles sectioning and mesh ops | `design/discussions/brep-geometry.md` #6 |
| Drop `dxf-rs`, `usvg` from Cargo.toml | Replaced by OpenCascade for geometry construction. Still need Rust crates for *parsing* DXF/SVG, then build OCCT wires from parsed entities. | `design/discussions/opencascade-bindings.md` |

## Rejected Features

| Feature | Reason | Source |
|---------|--------|--------|
| Machine profiles | Too complex for target audience. Quick NC code generation is the priority. | `design/discussions/api.md` #2.1 |
| Material library | User problem. Tool data is material-specific; user overrides per-operation. | `design/discussions/api.md` #2.5 |
| Rest machining | User can do it manually. Not worth the complexity. | `design/discussions/api.md` #2.9 |
| Probing / measurement cycles | Out of scope. | `design/discussions/api.md` #2.10 |

## Open Decisions

| Decision | Options / Notes |
|----------|-----------------|
| DXF geometry grouping | SVG uses stroke color to group entities into operations. DXF supports entity color (ACI group code 62, or true color group code 420) but files are commonly authored with color set BYLAYER. Options: (1) entity color only, (2) layer name only, (3) layer color, (4) entity color with BYLAYER fallback to layer color. Must decide before implementing DXF import. |
| stdin piping | Two axes: (a) job YAML from stdin via `chipmunk -`; (b) geometry from stdin via `--geometry -`. Both are straightforward to support independently. Open question: inline geometry in the YAML (base64 or raw SVG under a `geometry_inline:` key) — avoids the dual-stdin problem for fully dynamic pipelines but changes the data model. Inline geometry is only needed for backlog items (Inkscape extension, Onshape integration), so it can be deferred. Decision: support `-` for job YAML and `--geometry -` for geometry in Phase 1, defer inline geometry. |

## Deferred

| Item | Status | Source |
|------|--------|--------|
| API endpoint response examples | Fill in during implementation | `design/discussions/deferred-ideas.md` |
| Face/feature selection details | Partially resolved by B-rep face_ids. Tangent face grouping still open. | `design/discussions/deferred-ideas.md` |
| Setup sheets / job documentation | Revisit after Phase 4 (2.5D milling complete) | `design/discussions/deferred-ideas.md` |
| Simulation API contracts | Defined in `deferred/02-api-design.md` as future placeholders (deferred) | `design/discussions/api.md` #2.8 |
| Frontend | Browser UI, Three.js viewport, toolpath visualization | `design/tasks/backlog.md` |
| 3D projects | STEP/STL import, 3D viewport, B-rep slicer | `design/tasks/backlog.md` |
