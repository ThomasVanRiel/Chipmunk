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
core/               Tool, Operation, ToolpathSegment (data types)
    │
    ▼
toolpath/           Generate move sequences from operations
    │                (e.g. drill.rs produces rapids to each XY point)
    ▼
nc/compiler.rs      Wrap toolpath in envelope (tool change, spindle, clearance)
    │                → Vec<NCBlock> (intermediate representation)
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
├── cli/                CLI adapter (calls core, toolpath, nc)
├── core/               Data types: Tool, Operation, Toolpath, Units
│                       No framework deps — pure Rust
├── io/                 YAML parsing (job.rs)
├── toolpath/           Toolpath generation (drill.rs, future: profile, pocket)
│                       Depends on core/ only
└── nc/
    ├── ir.rs           NCBlock enum (the IR)
    ├── compiler.rs     Toolpath → NCBlock conversion
    ├── bridge.rs       Rust ↔ Lua bridge (mlua)
    └── postprocessors.rs  PP discovery (built-in + user dirs)

postprocessors/
├── base.lua            Shared helpers (loaded as globals)
└── heidenhain.lua      Built-in Heidenhain PP
```

### Dependency Rules

```
cli/        → core/, toolpath/, nc/, io/
core/       → (nothing — only std, serde, nalgebra)
toolpath/   → core/
nc/         → core/
io/         → core/
```

No circular dependencies. `core/`, `toolpath/`, `nc/`, `io/` have zero framework dependencies — no clap, no axum. They are independently testable.

## Envelope/Body Split

The NC compiler separates the **envelope** (tool change, spindle on, clearance rapid, spindle off) from the **body** (operation-specific blocks). Currently `compiler.rs` produces the full block list for manual drill; in the planned architecture, the compiler wraps any operation type's body with the standard envelope.

This means post-processors don't need to worry about whether tool changes or spindle commands are in the right place — the compiler handles that. The PP just formats whatever blocks it receives.

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
