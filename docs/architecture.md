# Architecture

This is a high-level overview for contributors. Detailed design lives in `design/docs/` — this page orients you and links there.

## Data Flow

```
YAML job file
    │
    ▼
io/job.rs           Parse YAML → JobConfig → OperationConfig.into_operation()
    │
    ▼
operations/         operation.generate() → ToolpathSegments
    │               operation.compile(segments) → NCBlocks
    │               (each operation type owns both steps)
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
├── core/               Data types: Tool, ToolpathSegment, Units, Locations
│                       No framework deps — pure Rust + serde
├── io/                 YAML parsing (job.rs)
│                       Deserializes OperationConfig, converts to Operation
├── operations/         Per-operation-type behavior
│   ├── mod.rs          Operation, OperationCommon, OperationKind, OperationType trait, dispatch
│   ├── quill.rs        Manual quill drilling: generate() + compile()
│   └── ...             Future: drill.rs (canned cycles), pocket.rs, profile.rs, etc.
└── nc/
    ├── ir.rs           NCBlock enum (the IR)
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

Operations use a shared struct (`OperationCommon`) for fields every operation needs, an enum (`OperationKind`) for type dispatch, and a trait (`OperationType`) for per-type behavior:

```rust
pub struct Operation<'a> {
    pub common: OperationCommon<'a>,
    pub kind: OperationKind,
}

pub struct OperationCommon<'a> {
    pub name: String,
    pub tool: Tool,
    pub capabilities: &'a PostprocessorCapabilities,
    pub clearance: f64,
}

pub enum OperationKind {
    Quill(Quill),
    // Future: Peck(Peck), Pocket(Pocket), Profile(Profile), ...
}

pub trait OperationType {
    fn generate(&self, common: &OperationCommon) -> Result<Vec<ToolpathSegment>>;
    fn compile(&self, common: &OperationCommon, segments: &[ToolpathSegment]) -> Result<Vec<NCBlock>>;
}
```

Each operation type is a self-contained struct that implements `OperationType`. `generate()` produces toolpath segments from the operation's parameters; `compile()` turns those segments into NCBlocks. Both receive `&OperationCommon` for shared fields (tool, clearance, etc.).

### Dispatch

A single private method on `Operation` bridges the enum to the trait:

```rust
impl Operation {
    fn kind_impl(&self) -> &dyn OperationType {
        match &self.kind {
            OperationKind::Quill(q) => q,
        }
    }

    pub fn generate(&self) -> Result<Vec<ToolpathSegment>> {
        self.kind_impl().generate(&self.common)
    }

    pub fn compile(&self, segments: &[ToolpathSegment]) -> Result<Vec<NCBlock>> {
        self.kind_impl().compile(&self.common, segments)
    }
}
```

Callers only see `operation.generate()` and `operation.compile()` — the trait and enum are internal. The enum match exists once in `kind_impl`; the compiler forces a new arm when a variant is added.

### Adding a new operation type

1. Create a struct with the type-specific fields (e.g. `Pocket { geometry, stepover, stepdown }`).
2. Implement `OperationType` — write `generate()` and `compile()`.
3. Add the variant to `OperationKind` — the compiler forces a match arm in `kind_impl`.
4. Add the serde variant to `OperationConfig` in `io/job.rs`.

### Code duplication is intentional

Each operation's `generate()` and `compile()` are self-contained. If two operations share similar logic (e.g. multiple drilling strategies building NC blocks with the same envelope structure), shared helpers can be extracted within the drilling family — but operations should not share behavior through abstraction layers. Repeated code across operation types is acceptable and preferred over premature generalization. Each operation module should be readable on its own.

See [discussion-operation-type-modeling.md](discussion-operation-type-modeling.md) for the full analysis behind this decision.

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
