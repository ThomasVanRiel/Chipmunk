# Name Proposals

**Current working title**: `CAMproject`

---

## Criteria

- Short enough to type as a CLI command without friction
- Memorable and distinct from existing tools (LinuxCNC, FreeCAD, Fusion, HSMWorks, etc.)
- Ideally hints at what it does without being generic (`ncgen`, `millcode`)
- Works as both the tool name and the binary name

---

## Proposals

### Machining vocabulary

| Name | Rationale |
|------|-----------|
| **swarf** | The metal chips and shavings produced during milling — very specific to machining, unusual as a software name, short, memorable. `swarf drill holes.dxf` |
| **spindle** | The rotating element of a milling machine. Evokes the machine directly. Slightly long for a CLI command. |
| **rapids** | Rapid moves are the fastest positioning moves in NC programs — the skeleton of every toolpath. Also suggests speed. `rapids mill part.svg` |
| **quill** | The vertical drilling mechanism on a knee mill. Also a writing/scripting pun — the tool that writes NC code. Fits the manual drill workflow well. `quill drill holes.dxf` |
| **stepdown** | The incremental Z depth per pass — a parameter every operator sets. Recognisable jargon but longer as a command. |
| **clearance** | The safe height above the part for rapid moves. Precise machining term but abstract as a name. |
| **feedrate** | Too compound, hard to type. |

### Craft / maker angle

| Name | Rationale |
|------|-----------|
| **millwright** | A millwright designs and builds mills. Evokes craft and precision. Two syllables as a command: `millwright`. Slightly formal. |
| **chipmaker** | Making chips is what milling does. Informal, descriptive. |
| **cutwright** | Invented — `wright` = maker/craftsman. `cutwright mill part.svg` |

### NC / output focused

| Name | Rationale |
|------|-----------|
| **postmill** | Post-processor + milling. Emphasises the NC output side. |
| **ncpath** | NC + path. Descriptive but dry. |
| **hcode** | Heidenhain uses `.H` files. Too narrow — implies Heidenhain-only. |

### Wordplay

| Name | Rationale |
|------|-----------|
| **chipmonk** | Chipmunk + chip (machining swarf). Playful, memorable, easy to spell. `chipmonk drill holes.dxf` |

### Short / coined

| Name | Rationale |
|------|-----------|
| **camr** | CAM + r (Rust). Minimal. Could be confused with the R language. |
| **milr** | Mill + r. Very short. No existing tool by this name. `milr drill holes.dxf` |
| **ncr** | NC + r. Extremely short. Reads as an abbreviation, no personality. |
| **forge** | Manufacturing connotation (forging metal). Already used by several other projects (`cargo`, `gitforge`, etc.). |

---

## Shortlist

| Name | CLI example | Notes |
|------|-------------|-------|
| **swarf** | `swarf drill holes.dxf --postprocessor heidenhain` | Most distinctive. Unmistakably machining. |
| **quill** | `quill mill part.svg --params job.yaml` | Dual meaning (drill mechanism + writing). Suits the manual drill origin. |
| **rapids** | `rapids mill part.svg --params job.yaml` | Energetic, recognisable to CNC operators. |
| **millwright** | `millwright drill holes.dxf` | Craft connotation, slightly long. |

---

## Notes

- Check crates.io and GitHub for name conflicts before committing.
- The binary name and the crate name can differ if needed (`camproject` crate, `swarf` binary).
- No final decision yet — `CAMproject` remains the working title.
