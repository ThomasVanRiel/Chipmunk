# External Software Integrations

## Overview

Chipmunk should be able to connect to external CAD systems to import geometry directly, rather than requiring manual file export/import. This is especially valuable for parametric workflows where the CAD model changes frequently — the CAM setup should update without re-importing files.

## Integration Architecture

```
┌────────────────────┐     ┌──────────────────────┐
│  External CAD      │     │     Chipmunk        │
│  ┌──────────────┐  │     │  ┌────────────────┐   │
│  │   Onshape    │──┼─API─┼─→│ integrations/  │   │
│  │   FreeCAD    │──┼─────┼─→│   onshape.rs   │   │
│  │   Fusion 360 │──┼─────┼─→│   freecad.rs   │   │
│  │   SolidWorks │──┼─────┼─→│   ...          │   │
│  └──────────────┘  │     │  └───────┬────────┘   │
└────────────────────┘     │          ↓             │
                           │  ┌────────────────┐   │
                           │  │    io/          │   │
                           │  │  stl_reader.rs  │   │
                           │  │  step_reader.rs │   │
                           │  └───────┬────────┘   │
                           │          ↓             │
                           │  ┌────────────────┐   │
                           │  │ core/geometry   │   │
                           │  └────────────────┘   │
                           └──────────────────────┘
```

The integration layer sits **above** the existing `io/` readers. It handles authentication, API communication, and model selection — then passes the retrieved geometry (as STEP or STL bytes) to the existing readers, which use OpenCascade to produce `TopoDS_Shape` B-rep geometry. This means the core CAM logic never knows or cares where the geometry came from.

## Integration Interface

```rust
// integrations/mod.rs

#[async_trait]
pub trait CADIntegration: Send + Sync {
    /// Human-readable name, e.g. "Onshape".
    fn name(&self) -> &str;

    /// Authenticate with the external service.
    async fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<bool>;

    /// List available documents/projects.
    async fn list_documents(&self) -> Result<Vec<CADDocument>>;

    /// List parts/bodies in a document.
    async fn list_parts(&self, document_id: &str) -> Result<Vec<CADPart>>;

    /// Export a part as geometry data.
    async fn export_part(
        &self,
        document_id: &str,
        part_id: &str,
        format: &str,  // "step", "stl", or "parasolid"
    ) -> Result<Vec<u8>>;

    /// Check if the part has been modified since last import.
    async fn check_for_updates(
        &self,
        document_id: &str,
        part_id: &str,
        last_version: &str,
    ) -> Result<bool>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CADDocument {
    pub id: String,
    pub name: String,
    pub last_modified: String,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CADPart {
    pub id: String,
    pub name: String,
    pub version: String,       // Version/revision identifier for change detection
    pub body_type: String,     // "solid", "surface", "assembly"
}
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
2. User browses their Onshape documents in a dialog within Chipmunk
3. User selects a part → Chipmunk fetches it as STEP (preferred) or STL
4. The geometry is passed to `step_reader.rs` or `stl_reader.rs` → `TopoDS_Shape` → `PartGeometry`
5. The `PartGeometry` stores the Onshape reference (document ID, part ID, version)
6. On "refresh", Chipmunk checks if the version has changed and re-imports if so

### Implementation

```rust
// integrations/onshape.rs

pub struct OnshapeIntegration {
    base_url: String,  // "https://cad.onshape.com/api/v6"
    access_key: String,
    secret_key: String,
    client: reqwest::Client,
}

#[async_trait]
impl CADIntegration for OnshapeIntegration {
    fn name(&self) -> &str { "Onshape" }

    async fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<bool> {
        // Verify credentials by calling a lightweight endpoint
        // Onshape uses HMAC-based request signing for API key auth
        ...
    }

    async fn list_documents(&self) -> Result<Vec<CADDocument>> {
        // GET /api/v6/documents
        ...
    }

    async fn list_parts(&self, document_id: &str) -> Result<Vec<CADPart>> {
        // GET /api/v6/partstudios/{did}/w/{wid}/parts
        ...
    }

    async fn export_part(&self, document_id: &str, part_id: &str, format: &str) -> Result<Vec<u8>> {
        // GET /api/v6/partstudios/{did}/w/{wid}/step (or /stl)
        ...
    }

    async fn check_for_updates(&self, document_id: &str, part_id: &str, last_version: &str) -> Result<bool> {
        // Compare current microversion with stored version
        ...
    }
}
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

### Approach

Use FreeCAD CLI (`freecadcmd`) as a subprocess to convert `.FCStd` to STEP/STL. This is simpler and more portable than trying to link FreeCAD's libraries directly.

```rust
// integrations/freecad.rs

pub struct FreeCADIntegration {
    freecad_path: PathBuf,  // Path to freecadcmd binary
}

#[async_trait]
impl CADIntegration for FreeCADIntegration {
    async fn export_part(&self, document_id: &str, part_id: &str, format: &str) -> Result<Vec<u8>> {
        // document_id = file path to .FCStd file
        // Run freecadcmd with a Python script to export as STEP/STL
        let output = tokio::process::Command::new(&self.freecad_path)
            .args(&["--run-script", &export_script_path])
            .output()
            .await?;
        ...
    }
}
```

### Workflow

1. User opens a `.FCStd` file (or browses for one)
2. Chipmunk calls FreeCAD CLI to list bodies and export as STEP
3. Geometry goes through `step_reader.rs` as usual

## Fusion 360 / SolidWorks (Future)

These are closed-source with limited API access:

- **Fusion 360**: Has a REST API (similar to Onshape) but requires Autodesk account and is less open. Could follow the same pattern as Onshape integration.
- **SolidWorks**: Desktop-only, COM API on Windows. Integration would require either a local bridge process or manual STEP/STL export. Lower priority.

## Generic Integration: Watch Folder

For CAD systems without API access, a simple "watch folder" integration:

1. User configures a folder path
2. Chipmunk watches for new/modified STL/STEP/DXF files in the folder
3. On change, the file is automatically re-imported
4. Works with any CAD that can "save as" to a folder

This is the simplest integration path and works universally. Many CAD tools support auto-export to a directory on save.

```rust
// integrations/watch_folder.rs

pub struct WatchFolderIntegration {
    watch_path: PathBuf,
    poll_interval: Duration,
}

impl WatchFolderIntegration {
    /// Start polling for file changes. Emits events via WebSocket.
    pub async fn start_watching(&self, tx: broadcast::Sender<FileChangeEvent>) {
        ...
    }
}
```

## Part Provenance

When geometry is imported from an external source, the `PartGeometry` stores provenance metadata:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartProvenance {
    pub source_type: String,                // "file", "onshape", "freecad", "watch_folder"
    pub source_path: Option<String>,        // File path (for file/watch_folder)
    pub source_document_id: Option<String>, // External document ID (for onshape/freecad)
    pub source_part_id: Option<String>,     // External part ID
    pub source_version: Option<String>,     // Version/revision at time of import
    pub imported_at: String,                // ISO 8601 timestamp
}
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
