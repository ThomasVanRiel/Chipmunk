# Deferred Ideas

Items discussed during the API review that were explicitly deferred. These are not forgotten — they're parked for later consideration.

---

## API endpoint examples (from review item 1.4)

These endpoints are listed in `02-api-design.md` but have no request/response examples yet. Fill in when implementing the relevant phase.

| Endpoint | What's needed |
|----------|--------------|
| `GET /api/project/parts/{id}` | Response format — bounding box, provenance, transform, update history? |
| `GET /api/project/parts/{id}/contour?z={z}` | Response format — GeoJSON, custom polygon format, coordinate rings? |
| `GET .../operations/{id}/toolpath/stats` | Clarify if redundant with stats in toolpath response, or a lightweight alternative |
| `POST .../operations/{id}/duplicate` | Does it accept overrides (e.g., name)? Or always a pure copy? |

---

## Face/feature selection for orientation (from review item 1.7)

**Partially resolved by B-rep decision.** With B-rep as primary geometry, each tessellated triangle carries a `face_id` mapping back to the original B-rep face. Click-face → orient works cleanly:

1. Three.js raycast → hit triangle → `face_ids[triangle_index]` → B-rep face ID
2. Send face ID to `POST /api/project/parts/{id}/orient`
3. Backend reads face normal from B-rep, computes orientation transform

**Still open — tangent face grouping for selection UI:**
- Individual B-rep faces are small (a single fillet surface, a single planar patch). Users may want to select a "logical face" that spans multiple B-rep faces.
- Tangent-continuous face groups (e.g., a flat surface + its surrounding fillets) are useful for machining but hard to define generically.
- For now, individual B-rep face selection is sufficient for orientation. Logical face grouping can be revisited when needed for operation-specific face selection (e.g., "machine this face").

---

## Setup sheets / job documentation (from review item 2.7)

Operators need a printed reference sheet at the machine with tool list, operation sequence, WCS info, stock dimensions, and estimated cycle time. Fusion 360 generates HTML; Mastercam has templates.

Proposed endpoint: `GET /api/project/setup-sheet?format=html`

Deferred — revisit after NC export is working (Phase 3+).
