# Frontend Design

## Overview

The frontend is a single-page application using Three.js for 3D visualization and vanilla TypeScript for UI logic. No heavy framework (React/Vue/Svelte) — the UI is simple enough that direct DOM manipulation keeps the bundle small and avoids framework churn.

If the UI grows more complex in later phases, migrating to a lightweight framework (e.g., Preact, Lit) can be considered.

## Layout

```
┌──────────────────────────────────────────────────────────────┐
│  Toolbar: [Open] [Undo] [Redo] [Add Op ▼] [Generate] [Export] │
├────────────────────────────────────┬─────────────────────────┤
│                                    │  Tab: Operations        │
│                                    │  ┌───────────────────┐  │
│                                    │  │ ☑ 1. Facing       │  │
│                                    │  │ ☑ 2. Rough pocket │  │
│         3D Viewport                │  │ ☐ 3. Finish pass  │  │
│         (Three.js)                 │  └───────────────────┘  │
│                                    │                         │
│                                    │  Tab: Properties        │
│                                    │  ┌───────────────────┐  │
│                                    │  │ Type: Pocket      │  │
│                                    │  │ Tool: 6mm EM      │  │
│                                    │  │ Depth: -10mm      │  │
│                                    │  │ Stepover: 40%     │  │
│                                    │  │ Strategy: Contour │  │
│                                    │  └───────────────────┘  │
│                                    │                         │
│                                    │  Tab: Tools             │
│                                    │  Tab: NC Preview        │
├────────────────────────────────────┴─────────────────────────┤
│  Status bar: [Units: mm] [Toolpath: 1234 segments, ~12 min] │
└──────────────────────────────────────────────────────────────┘
```

- **3D Viewport** (center): Fills most of the screen. Renders part mesh, toolpath lines, grid, axes. Stock wireframe when defined.
- **Sidebar** (right, resizable): Tabbed panels for operations, properties, tools, NC preview.
- **Toolbar** (top): Primary actions. Includes Undo/Redo buttons (also Ctrl+Z / Ctrl+Shift+Z).
- **Status bar** (bottom): Units display, toolpath stats, progress during generation.

## 3D Viewport

### Scene Setup

```typescript
// Three.js scene components
scene: THREE.Scene
camera: THREE.PerspectiveCamera
renderer: THREE.WebGLRenderer
controls: THREE.OrbitControls

// Scene objects
gridHelper: THREE.GridHelper        // XY grid on Z=0
axesHelper: THREE.AxesHelper        // RGB XYZ axes
partMesh: THREE.Mesh                // Imported part geometry
stockWireframe: THREE.LineSegments  // Stock bounding box
toolpathLines: THREE.Group          // Toolpath visualization
```

### Rendering

**Part mesh**:
- `MeshPhongMaterial` with light gray color and slight transparency
- Ambient + directional light (from upper-left)
- Double-sided rendering (STL meshes may have inconsistent normals)

**Stock**:
- Wireframe box using `EdgesGeometry` + `LineBasicMaterial`
- Semi-transparent blue

**Toolpath**:
- Rapid moves: red dashed lines (`LineDashedMaterial`)
- Feed moves (XY): blue solid lines
- Plunge/retract moves (Z-dominant): green solid lines
- Arc moves: tessellated into line segments (Three.js doesn't have arc primitives for lines)

**Grid**:
- XY plane grid at Z=0
- Major lines every 10mm, minor lines every 1mm (at close zoom)
- Subdued gray color

### Camera Controls

- **Left mouse drag**: Orbit (rotate around pivot point)
- **Middle mouse drag**: Pan
- **Scroll wheel**: Zoom
- **Fit to view** (toolbar button or 'F' key): Frame all visible geometry
- **Standard views** (toolbar or numpad): Top (XY), Front (XZ), Right (YZ), Isometric

### Mouse Interaction

- **Click face**: Select a face on the mesh — used for:
  - Part orientation: "set this face as top" (orients Z+ to face normal, Z=0 at face surface)
  - WCS placement: "set origin here"
  - Operation geometry selection: "machine this face/pocket/contour"
- **Click edge**: Select an edge for profile operations
- **Click toolpath segment**: Highlight and show info (position, feed, move type)
- **Right-click**: Context menu (add operation on selected face, set as top, set WCS here)

## Sidebar Panels

### Operations Panel

Lists all operations in execution order. Each operation shows:
- **Eye icon**: Toggle toolpath visibility in viewport (per-operation show/hide)
- Checkbox (enable/disable in NC output)
- Sequence number
- Operation name and type icon
- Status indicator (no toolpath / generated / error)

Actions:
- **Add**: Dropdown with operation types (Facing, Profile, Pocket, Drill)
- **Duplicate**: Copy selected operation with all parameters (name suffixed " (copy)")
- **Delete**: Remove selected operation
- **Move Up/Down**: Reorder operations
- **Generate**: Generate toolpath for selected operation
- **Generate All**: Generate all toolpaths

Selecting an operation:
- Highlights its toolpath in the 3D view (others dimmed but still visible if eye icon is on)
- Shows its properties in the Properties panel

### Properties Panel

Shows parameters for the selected operation. Fields update via the API when changed.

Organized in collapsible groups (essentials expanded, advanced collapsed):
- **General**: Name, type (read-only), enabled, optional (block delete)
- **WCS**: Origin XYZ, rotation ABC, work offset (G54-G59), "pick on model" button
- **Geometry**: Part reference, start depth, final depth
- **Tool & Cutting**: Tool selection (from project tools), feed rate, plunge rate, spindle speed
- **Depth**: Depth per pass, depth strategy
- **Type-specific**: Stepover (facing/pocket), profile side, cut direction, compensation mode, tabs, pocket strategy
- **Machine Control** (collapsed): Stop before/after (None / M0 / M1), skip level

### Tools Panel

Lists all defined tools with:
- Name, type, diameter
- Add / Edit / Delete buttons

Tool editor (dialog or inline):
- Tool type selection
- Dimensions: diameter, flute length, total length
- Cutting parameters: feed rate, plunge rate, spindle speed, depth per pass, stepover

### NC Preview Panel

Shows the generated NC code as syntax-highlighted text:
- G-code keywords in blue
- Coordinates in black
- Comments in green
- Line numbers in gray

Controls:
- Post-processor selection dropdown
- Operation filter (which operations to include)
- Copy to clipboard button
- Download button

## Frontend Build

### Tooling

- **Vite** as dev server and bundler (fast HMR, TypeScript support, minimal config)
- **TypeScript** for type safety
- **Three.js** imported as ES module

### Development Workflow

```bash
# Terminal 1: Backend
cargo run -- --dev --port 8000       # Axum with CORS enabled, API only

# Terminal 2: Frontend
cd frontend && npm run dev           # Vite dev server with proxy to backend
```

Vite proxies `/api/*` to the axum backend during development.

For production, the frontend is built (`npm run build`) and the axum server serves the static files from `frontend/dist/`.

### File Structure

```
frontend/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/
│   ├── main.ts                 # Entry: init app, set up API connection
│   ├── api.ts                  # REST client + WebSocket manager
│   ├── types.ts                # TypeScript types matching backend models
│   ├── viewport/
│   │   ├── scene.ts            # Three.js scene, renderer, lights setup
│   │   ├── camera.ts           # OrbitControls configuration
│   │   ├── mesh-loader.ts      # Fetch mesh from API → Three.js BufferGeometry
│   │   ├── toolpath-renderer.ts # Fetch toolpath → colored line segments
│   │   ├── stock-renderer.ts   # Wireframe stock box
│   │   └── grid.ts             # Grid and axes helpers
│   ├── panels/
│   │   ├── operations.ts       # Operation list panel
│   │   ├── properties.ts       # Property editor panel
│   │   ├── tools.ts            # Tool library panel
│   │   └── nc-preview.ts       # NC code preview panel
│   ├── dialogs/
│   │   ├── stock-setup.ts      # Stock definition dialog
│   │   └── tool-editor.ts      # Tool add/edit dialog
│   └── utils/
│       ├── dom.ts              # DOM helper utilities
│       └── format.ts           # Number/unit formatting
└── styles/
    └── main.css                # Application styles
```

## API Client

```typescript
// api.ts - simplified interface

class CAMApi {
  // Project
  async createProject(name: string, units: string): Promise<Project>
  async getProject(): Promise<Project>

  // Parts
  async importFile(file: File): Promise<Part>
  async getPartMesh(partId: string): Promise<MeshData>

  // Tools
  async getTools(): Promise<Tool[]>
  async addTool(tool: Tool): Promise<Tool>
  async updateTool(id: string, tool: Tool): Promise<Tool>
  async deleteTool(id: string): Promise<void>

  // Operations
  async getOperations(): Promise<Operation[]>
  async addOperation(op: Operation): Promise<Operation>
  async updateOperation(id: string, op: Operation): Promise<Operation>
  async deleteOperation(id: string): Promise<void>
  async generateToolpath(opId: string): Promise<void>  // triggers WebSocket progress

  // Toolpath
  async getToolpath(opId: string): Promise<ToolpathData>

  // NC Export
  async getPostProcessors(): Promise<PostProcessor[]>
  async previewNC(postId: string, opIds?: string[]): Promise<string>
  async exportNC(postId: string, opIds?: string[]): Promise<Blob>
}

class CAMWebSocket {
  onProgress(callback: (data: ProgressData) => void): void
  onComplete(callback: (data: CompleteData) => void): void
  onError(callback: (data: ErrorData) => void): void
  onProjectUpdated(callback: (data: UpdateData) => void): void
}
```

## Responsive Design

The layout adapts to window size:
- **Wide screens** (>1200px): Sidebar visible, full 3D viewport
- **Medium screens** (800-1200px): Collapsible sidebar
- **Narrow screens** (<800px): Sidebar as overlay/drawer

The viewport always fills available space and re-renders on resize.
