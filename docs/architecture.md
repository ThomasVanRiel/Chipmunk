# Architecture

This is a high-level overview for contributors. Detailed design lives in `design/docs/` — this page orients you and links there.

## Data Flow

```
YAML job file
    │
    ▼
io/job.rs           Parse YAML into JobConfig
    │
    ▼
core/               Tool, ToolpathSegment, NCBlock IR (data types)
    │
    ▼
operations/         Per-type generate() + compile()
    │                (e.g. drill.rs generates toolpath, then compiles to NCBlocks)
    ▼
nc/bridge.rs        Fresh Lua VM per call
    │                Load base.lua + post-processor
    │                Convert NCBlocks to Lua tables
    │                Call M.generate(blocks, context)
    ▼
NC string           → stdout or --output file
```

## Three-Layer NC Generation

1. **Toolpath** — pure geometry: rapid/linear segments with XYZ coordinates. No machine concepts.
2. **NCBlock IR** — controller-neutral program: adds tool changes, spindle, coolant, compensation, cycles. Defined in `src/nc/ir.rs`.
3. **Post-processor** — formats IR into machine-specific output (G-code, Heidenhain conversational, etc.). Written in Lua.

Toolpath generators never know about G-code. Post-processors never know about machining strategies. The IR is the boundary.

## Module Structure

```
src/
├── bin/chipmunk.rs     CLI entry point (clap)
├── lib.rs              Crate root
├── cli/                CLI adapter (calls io/, operations/, nc/)
├── core/               Data types: Tool, ToolpathSegment, NCBlock IR, Units
│                       No framework deps — pure Rust + serde
├── io/                 YAML parsing (job.rs)
│                       Builds Operation structs, dispatches to operations/
├── operations/         Per-operation-type behavior
│   ├── mod.rs          OperationKind enum, trait defs, dispatch
│   ├── drill.rs        generate() + compile() for drilling
│   ├── patterns.rs     Shared hole patterns (bolt circle, grid, etc.)
│   └── ...             Future: pocket.rs, profile.rs, face.rs, tap.rs
└── nc/
    ├── ir.rs           NCBlock enum (the IR) — lives in core/ conceptually
    ├── bridge.rs       Rust ↔ Lua bridge (mlua)
    └── postprocessors.rs  PP discovery (built-in + user dirs)
```

### Dependency Rules

```
cli/          → io/, operations/, nc/
io/           → core/, operations/
operations/   → core/
nc/           → core/
core/         → (nothing — only std, serde, nalgebra)
```

No circular dependencies. `core/`, `operations/`, `nc/`, `io/` have zero framework dependencies — no clap, no axum. They are independently testable.

**Key constraints:**
- `operations/` depends only on `core/` — it reads data types and produces data types. It never touches IO, Lua, or the CLI.
- `nc/` depends only on `core/` — it takes NCBlocks and formats them via Lua. It never knows about operation types.
- `io/` is the integration layer — it parses YAML, builds `Operation` structs, calls `operations/` to generate and compile, then hands NCBlocks to `nc/` for post-processing.

## Operation Architecture

Operations use a shared struct for common fields and an enum for type-specific parameters:

```rust
pub struct Operation {
    pub name: String,
    pub tool: Tool,
    pub clearance: f64,
    pub skip_level: Option<u8>,
    pub kind: OperationKind,
}

pub enum OperationKind {
    Drill { strategy: DrillStrategy, locations: Locations, depth: f64 },
    Pocket { geometry: GeometryRef, stepover: f64, stepdown: f64 },
    // ...
}
```

`Operation` methods match on `kind` and delegate to per-type modules. No traits needed — the operation set is closed at compile time, so the match is the dispatch:

```rust
impl Operation {
    pub fn generate(&self) -> Vec<ToolpathSegment> {
        match &self.kind {
            OperationKind::Drill { .. } => drill::generate(self),
            OperationKind::Pocket { .. } => pocket::generate(self),
            // ...
        }
    }

    pub fn compile(&self, segments: &[ToolpathSegment]) -> Vec<NCBlock> {
        match &self.kind {
            OperationKind::Drill { .. } => drill::compile(self, segments),
            OperationKind::Pocket { .. } => pocket::compile(self, segments),
            // ...
        }
    }
}
```

Per-type functions receive `&Operation` and extract what they need from `kind` internally. Adding a new operation type requires:

1. Add the variant to `OperationKind`.
2. Create the per-type module with `generate()` and `compile()`.
3. Add the match arm in each `Operation` method — the compiler enforces this.

See [discussion-operation-type-modeling.md](discussion-operation-type-modeling.md) for the full analysis behind this decision.

## Envelope/Body Split

The NC compiler separates the **envelope** (tool change, spindle on, clearance rapid, spindle off) from the **body** (operation-specific blocks). The `compile()` trait method for each operation type produces the body; the envelope wrapping is shared logic.

This means post-processors don't need to worry about whether tool changes or spindle commands are in the right place — the envelope handles that. The PP just formats whatever blocks it receives.

## Post-Processor Execution

- A fresh Lua VM is created for each NC generation call (cheap — microseconds).
- `base.lua` is loaded first, making its functions available as globals.
- The PP file is loaded and evaluated — it must return a module table.
- `M.generate(blocks, context)` is called with the IR as Lua tables.
- The returned string is the NC program.

See [postprocessors.md](postprocessors.md) for the full PP authoring guide and IR reference.

## Design Documents

| Doc | Contents |
|-----|----------|
| `design/docs/00-overview.md` | Architecture, tech choices, design principles |
| `design/docs/01-data-model.md` | Core types: Tool, Operation, Toolpath |
| `design/docs/03-nc-and-postprocessors.md` | NCBlock IR, Lua bridge, drill strategies, canned cycles |
| `design/docs/04-toolpath-algorithms.md` | Slicing, offset, profile, pocket algorithms |
| `design/docs/06-project-structure.md` | Directory tree, Cargo.toml |
| `design/docs/07-implementation-phases.md` | Phased task breakdown |
| `design/docs/11-plugin-system.md` | PP plugin mechanics, operation type registry |
