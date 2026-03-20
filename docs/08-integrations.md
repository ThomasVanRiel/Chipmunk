# External Software Integrations

## Overview

CAMproject should be able to connect to external CAD systems to import geometry directly, rather than requiring manual file export/import. This is especially valuable for parametric workflows where the CAD model changes frequently — the CAM setup should update without re-importing files.

## Integration Architecture

```
┌────────────────────┐     ┌──────────────────────┐
│  External CAD      │     │     CAMproject        │
│  ┌──────────────┐  │     │  ┌────────────────┐   │
│  │   Onshape    │──┼─API─┼─→│  integrations/ │   │
│  │   FreeCAD    │──┼─────┼─→│    onshape.py  │   │
│  │   Fusion 360 │──┼─────┼─→│    freecad.py  │   │
│  │   SolidWorks │──┼─────┼─→│    ...         │   │
│  └──────────────┘  │     │  └───────┬────────┘   │
└────────────────────┘     │          ↓             │
                           │  ┌────────────────┐   │
                           │  │    io/          │   │
                           │  │  stl_reader.py  │   │
                           │  │  step_reader.py │   │
                           │  └───────┬────────┘   │
                           │          ↓             │
                           │  ┌────────────────┐   │
                           │  │ core/geometry   │   │
                           │  └────────────────┘   │
                           └──────────────────────┘
```

The integration layer sits **above** the existing `io/` readers. It handles authentication, API communication, and model selection — then passes the retrieved geometry (as STL/STEP bytes or file) to the existing readers. This means the core CAM logic never knows or cares where the geometry came from.

## Integration Interface

```python
# integrations/base.py

class CADIntegration(ABC):
    """Base class for external CAD system integrations."""

    @property
    @abstractmethod
    def name(self) -> str:
        """Human-readable name, e.g. 'Onshape'."""
        ...

    @abstractmethod
    async def authenticate(self, credentials: dict) -> bool:
        """Authenticate with the external service."""
        ...

    @abstractmethod
    async def list_documents(self) -> list[CADDocument]:
        """List available documents/projects."""
        ...

    @abstractmethod
    async def list_parts(self, document_id: str) -> list[CADPart]:
        """List parts/bodies in a document."""
        ...

    @abstractmethod
    async def export_part(
        self,
        document_id: str,
        part_id: str,
        format: str = "step",  # "step", "stl", or "parasolid"
    ) -> bytes:
        """Export a part as geometry data."""
        ...

    async def check_for_updates(
        self,
        document_id: str,
        part_id: str,
        last_version: str,
    ) -> bool:
        """Check if the part has been modified since last import."""
        ...

@dataclass
class CADDocument:
    id: str
    name: str
    last_modified: str
    thumbnail_url: str | None

@dataclass
class CADPart:
    id: str
    name: str
    version: str             # Version/revision identifier for change detection
    body_type: str           # "solid", "surface", "assembly"
```

## Onshape Integration

Onshape is the highest-priority integration because:

- It's fully cloud-based with a well-documented REST API
- No local software install required — aligns with our browser-based approach
- Widely used in hobbyist and professional CNC workflows

### Onshape API

Onshape provides REST endpoints for:

- **Authentication**: OAuth2 or API keys (key pair: access key + secret key)
- **Document listing**: `GET /api/documents` — list user's documents
- **Part studios**: `GET /api/partstudios/{did}/{wvm}/{wvmid}/parts` — list parts
- **Export**: `GET /api/partstudios/{did}/{wvm}/{wvmid}/stl` or `/step` — export geometry
- **Thumbnails**: `GET /api/thumbnails/{did}` — document previews
- **Versioning**: Each document has workspaces/versions/microversions for change tracking

### Workflow

1. User connects their Onshape account (OAuth2 flow or API key entry)
2. User browses their Onshape documents in a dialog within CAMproject
3. User selects a part → CAMproject fetches it as STEP (preferred) or STL
4. The geometry is passed to `step_reader.py` or `stl_reader.py` → `PartGeometry`
5. The `PartGeometry` stores the Onshape reference (document ID, part ID, version)
6. On "refresh", CAMproject checks if the version has changed and re-imports if so

### Implementation

```python
# integrations/onshape.py

class OnshapeIntegration(CADIntegration):
    """Onshape REST API integration."""

    BASE_URL = "https://cad.onshape.com/api/v6"

    def __init__(self, access_key: str, secret_key: str):
        self.access_key = access_key
        self.secret_key = secret_key
        # Onshape uses HMAC-based request signing for API key auth

    async def authenticate(self, credentials: dict) -> bool:
        # Verify credentials by calling a lightweight endpoint
        ...

    async def list_documents(self) -> list[CADDocument]:
        # GET /api/v6/documents
        ...

    async def list_parts(self, document_id: str) -> list[CADPart]:
        # GET /api/v6/partstudios/{did}/w/{wid}/parts
        ...

    async def export_part(self, document_id: str, part_id: str, format: str = "step") -> bytes:
        # GET /api/v6/partstudios/{did}/w/{wid}/step (or /stl)
        ...

    async def check_for_updates(self, document_id: str, part_id: str, last_version: str) -> bool:
        # Compare current microversion with stored version
        ...
```

### API Endpoints for Integration

```
POST /api/integrations/onshape/connect       — store API credentials
GET  /api/integrations/onshape/documents     — list documents
GET  /api/integrations/onshape/documents/{id}/parts — list parts
POST /api/integrations/onshape/import        — import a part into the project
POST /api/integrations/onshape/refresh/{part_id} — check for updates and re-import
```

### Frontend

- "Import from Onshape" button in toolbar (alongside "Open File")
- Modal dialog: API key setup → document browser → part selection → import
- Imported parts show an Onshape icon badge and "Refresh from CAD" action

## FreeCAD Integration

FreeCAD is an important integration target because it's open-source and widely used.

### Approach Options

**Option A: FreeCAD Python library** — FreeCAD can be imported as a Python module. If FreeCAD is installed, `import FreeCAD` gives access to its document model. This allows reading `.FCStd` files directly.

```python
# integrations/freecad.py
import FreeCAD
import Part  # FreeCAD Part module

class FreeCADIntegration(CADIntegration):
    async def export_part(self, document_id: str, part_id: str, format: str = "step") -> bytes:
        doc = FreeCAD.open(document_id)  # document_id = file path
        shape = doc.getObject(part_id).Shape
        step_bytes = shape.exportStep()
        return step_bytes
```

**Option B: FreeCAD CLI** — Use `freecadcmd` as a subprocess to convert `.FCStd` to STEP/STL.

Option A is more tightly integrated but couples us to FreeCAD's Python environment. Option B is simpler and more portable. Start with Option B.

### Workflow

1. User opens a `.FCStd` file (or browses for one)
2. CAMproject calls FreeCAD CLI to list bodies and export as STEP
3. Geometry goes through `step_reader.py` as usual

## Fusion 360 / SolidWorks (Future)

These are closed-source with limited API access:

- **Fusion 360**: Has a REST API (similar to Onshape) but requires Autodesk account and is less open. Could follow the same pattern as Onshape integration.
- **SolidWorks**: Desktop-only, COM API on Windows. Integration would require either a local bridge process or manual STEP/STL export. Lower priority.

## Generic Integration: Watch Folder

For CAD systems without API access, a simple "watch folder" integration:

1. User configures a folder path
2. CAMproject watches for new/modified STL/STEP/DXF files in the folder
3. On change, the file is automatically re-imported
4. Works with any CAD that can "save as" to a folder

This is the simplest integration path and works universally. Many CAD tools support auto-export to a directory on save.

```python
# integrations/watch_folder.py

class WatchFolderIntegration:
    """Watch a directory for geometry file changes."""

    def __init__(self, watch_path: str, poll_interval: float = 2.0):
        ...

    async def start_watching(self):
        """Start polling for file changes. Emits events via WebSocket."""
        ...
```

## Project Structure Addition

```
src/camproject/
├── integrations/
│   ├── __init__.py
│   ├── base.py              # CADIntegration ABC, CADDocument, CADPart
│   ├── onshape.py           # Onshape REST API integration
│   ├── freecad.py           # FreeCAD file/CLI integration
│   └── watch_folder.py      # Generic file-watching integration
```

## Part Provenance

When geometry is imported from an external source, the `PartGeometry` stores provenance metadata:

```python
@dataclass
class PartProvenance:
    source_type: str                # "file", "onshape", "freecad", "watch_folder"
    source_path: str | None         # File path (for file/watch_folder)
    source_document_id: str | None  # External document ID (for onshape/freecad)
    source_part_id: str | None      # External part ID
    source_version: str | None      # Version/revision at time of import
    imported_at: str                 # ISO 8601 timestamp
```

This enables:

- "Refresh from CAD" — re-import from the same source
- "Is this up to date?" — compare stored version with current version
- Project portability — another user can see where geometry came from even without API access

## Implementation Priority

1. **File import** (Phase 1) — already planned, this is the baseline
2. **Watch folder** (Phase 3-4) — simple, works with everything
3. **Onshape** (Phase 4-5) — highest-value API integration
4. **FreeCAD** (Phase 5+) — important for open-source workflow
5. **Fusion 360 / SolidWorks** (future) — based on demand
