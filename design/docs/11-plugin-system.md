# Plugin System & Operation Type Registry

Chipmunk has one runtime plugin system (post-processors) and one compile-time type registry (toolpath operations):

| Extension point | Language | Extensible at runtime? |
|---|---|---|
| Post-processors | Lua (via mlua) | Yes — drop a `.lua` file in the config dir |
| Toolpath operations | Rust (compiled in) | No — add a new type, recompile |

Toolpath operations are not runtime-pluggable by design. They are geometry-heavy computation that must be fast, and the set of operation types is stable enough that recompilation is the right boundary. The trait system still provides a clean internal structure for adding new operation types during development.

---

## Post-Processor Plugins (Lua)

See `03-nc-and-postprocessors.md` for the full post-processor design. This section covers the plugin mechanics.

### How Plugins Are Loaded

**Built-in post-processors** are embedded at compile time:

```rust
// src/nc/postprocessors/mod.rs
pub const BUILTIN: &[(&str, &str)] = &[
    ("linuxcnc",   include_str!("../../../../postprocessors/linuxcnc.lua")),
    ("grbl",       include_str!("../../../../postprocessors/grbl.lua")),
    ("fanuc",      include_str!("../../../../postprocessors/fanuc.lua")),
    ("marlin",     include_str!("../../../../postprocessors/marlin.lua")),
    ("sinumerik",  include_str!("../../../../postprocessors/sinumerik.lua")),
    ("heidenhain", include_str!("../../../../postprocessors/heidenhain.lua")),
];
```

They are part of the binary — no files needed at runtime.

**User post-processors** are scanned at startup from:
```
Linux/macOS:  ~/.config/chipmunk/postprocessors/
Windows:      %APPDATA%\chipmunk\postprocessors\
```

Any `.lua` file found there is registered. The filename (minus `.lua`) becomes the ID. A user file with the same ID as a built-in overrides the built-in — this is how users can patch a built-in without recompiling.

### Registry

```rust
pub struct PostProcessorRegistry {
    // ID → Lua source string
    scripts: HashMap<String, String>,
}

impl PostProcessorRegistry {
    pub fn load() -> Result<Self> {
        let mut scripts = HashMap::new();

        // Built-ins first
        for (id, src) in BUILTIN {
            scripts.insert(id.to_string(), src.to_string());
        }

        // User overrides (may replace built-ins)
        let user_dir = user_postprocessors_dir();
        if user_dir.exists() {
            for entry in std::fs::read_dir(user_dir)? {
                let path = entry?.path();
                if path.extension() == Some("lua".as_ref()) {
                    let id = path.file_stem().unwrap().to_string_lossy().into_owned();
                    let src = std::fs::read_to_string(&path)?;
                    scripts.insert(id, src);
                }
            }
        }

        Ok(Self { scripts })
    }

    pub fn list(&self) -> Vec<PostProcessorInfo> { ... }
    pub fn get(&self, id: &str) -> Option<&str> { self.scripts.get(id).map(String::as_str) }
}
```

### Lua Execution Model

Each call to `generate_nc_code()` creates a fresh `Lua` instance. This avoids state leakage between calls (a post-processor that accidentally sets globals won't affect the next call). The cost is low — Lua VM startup is microseconds.

```rust
pub fn generate_nc_code(
    registry: &PostProcessorRegistry,
    blocks: &[NCBlock],
    context: &ProgramContext,
    id: &str,
) -> Result<String> {
    let src = registry.get(id).ok_or(Error::UnknownPostProcessor(id.to_string()))?;

    let lua = Lua::new();

    // Load base helpers (always available to post-processors via require("base"))
    let base_src = registry.get("base").expect("base.lua always present");
    lua.load(base_src).set_name("base").exec()?;

    // Load and evaluate the post-processor module
    let pp: LuaTable = lua.load(src).set_name(id).eval()?;

    // Build Lua arguments
    let blocks_table = ncblocks_to_lua(&lua, blocks)?;
    let context_table = context_to_lua(&lua, context)?;

    // Call generate()
    let generate: LuaFunction = pp.get("generate")?;
    let result: String = generate.call((blocks_table, context_table))
        .map_err(|e| Error::LuaError(id.to_string(), e.to_string()))?;

    Ok(result)
}
```

### Querying Post-Processor Capabilities

Before compiling, the NC compiler queries the post-processor for its capabilities (which cycle types it supports, which skip strategy it prefers). This is done by loading the module and reading its fields — no `generate()` call needed.

```rust
pub fn get_capabilities(registry: &PostProcessorRegistry, id: &str) -> Result<PostProcessorCapabilities> {
    let lua = Lua::new();
    let src = registry.get(id).ok_or(...)?;
    let pp: LuaTable = lua.load(src).eval()?;

    let supported_cycles: Vec<String> = pp.get::<Option<LuaTable>>("supported_cycles")?
        .map(|t| t.sequence_values().collect::<LuaResult<_>>())
        .transpose()?
        .unwrap_or_default();

    let skip_strategy: String = pp.get::<Option<String>>("optional_skip_strategy")?
        .unwrap_or_else(|| "block_delete".to_string());

    Ok(PostProcessorCapabilities { supported_cycles, skip_strategy })
}
```

### Testing Post-Processors

Post-processors are tested in `tests/test_postprocessors.rs`:

```rust
#[test]
fn linuxcnc_facing_output() {
    let registry = PostProcessorRegistry::load().unwrap();
    let blocks = fixtures::simple_facing_blocks();
    let context = fixtures::default_context();
    let output = generate_nc_code(&registry, &blocks, &context, "linuxcnc").unwrap();
    let expected = include_str!("fixtures/nc/linuxcnc_facing.ngc");
    assert_eq!(output, expected);
}
```

Golden files live in `tests/fixtures/nc/`. When a post-processor changes intentionally, update the golden file. This catches regressions in output formatting.

---

## Toolpath Operation Types (Rust, Compile-Time)

Toolpath generators are geometry-heavy. Pocket clearing runs hundreds of polygon offset iterations — interpreted code would be unacceptably slow. Operation types are plain Rust structs that implement a trait and are registered at compile time. There is no dynamic loading or runtime discovery.

### `OperationType` Trait

```rust
// src/toolpath/registry.rs

pub trait OperationType: Send + Sync + 'static {
    /// The string identifier for this operation type. Must match the
    /// `operation_type` field used in the API and data model.
    fn type_id(&self) -> &'static str;

    /// Human-readable name for the UI.
    fn display_name(&self) -> &'static str;

    /// Describe the parameters this operation type accepts.
    /// Used by the frontend to render the correct property fields
    /// and by the API to validate incoming operation requests.
    fn parameter_schema(&self) -> ParameterSchema;

    /// Validate the operation against the given geometry.
    /// Returns a list of user-facing issues. Empty = valid.
    /// Only return errors for physically impossible situations
    /// (tool wider than pocket, depth exceeds part, etc.).
    /// Do not warn about aggressive but valid parameters.
    fn validate(
        &self,
        op: &Operation,
        geometry: &PartGeometry,
    ) -> Vec<ValidationIssue>;

    /// Generate the toolpath for this operation.
    /// Called from spawn_blocking — may do heavy computation.
    /// Always produces explicit move segments — used for visualization
    /// and as the universal NC fallback when no cycle support is available.
    fn generate(
        &self,
        op: &Operation,
        geometry: &PartGeometry,
    ) -> Result<Vec<ToolpathSegment>, ToolpathError>;

    /// Optionally produce optimized NCBlocks for this operation instead of
    /// the generic toolpath→NCBlock conversion.
    ///
    /// Called by the NC compiler before falling back to generic segment
    /// conversion. Return Some(blocks) to use operation-specific output
    /// (e.g., canned cycles for drilling). Return None to fall through.
    ///
    /// Reads parameters directly from `op` — does NOT reconstruct intent
    /// from the generated move sequence. `caps` describes what the target
    /// post-processor supports (e.g., which cycle types).
    ///
    /// Default: None. Most operations (facing, profile, pocket) don't
    /// override this — they rely on generic segment conversion.
    fn compile_nc(
        &self,
        op: &Operation,
        caps: &PostProcessorCapabilities,
    ) -> Option<Vec<NCBlock>> {
        None
    }
}
```

### Parameter Schema

The `ParameterSchema` describes which fields an operation type uses. This drives:
- Frontend rendering (which property rows to show in the Properties panel)
- API validation (reject unknown fields, enforce required fields)
- Documentation generation

```rust
pub struct ParameterSchema {
    pub fields: Vec<FieldSchema>,
}

pub struct FieldSchema {
    pub name: &'static str,           // Field name on Operation struct
    pub display: &'static str,        // UI label, e.g. "Stepover"
    pub field_type: FieldType,        // Float, Int, Enum, Bool
    pub unit: Option<&'static str>,   // "mm", "mm/min", "RPM", "%", etc.
    pub required: bool,
    pub default: Option<serde_json::Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

pub enum FieldType {
    Float,
    Int,
    Bool,
    Enum(Vec<(&'static str, &'static str)>),  // (value, display)
}
```

### Registry

A static map from type ID string to type implementation. All entries are known at compile time — nothing is discovered or loaded at runtime.

```rust
// src/toolpath/registry.rs
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

type TypeMap = HashMap<&'static str, Arc<dyn OperationType>>;

static REGISTRY: LazyLock<TypeMap> = LazyLock::new(|| {
    let mut m: TypeMap = HashMap::new();
    m.insert(FacingOperation.type_id(),  Arc::new(FacingOperation));
    m.insert(ProfileOperation.type_id(), Arc::new(ProfileOperation));
    m.insert(PocketOperation.type_id(),  Arc::new(PocketOperation));
    // Phase 4: m.insert(DrillOperation.type_id(), Arc::new(DrillOperation));
    m
});

pub fn get(type_id: &str) -> Option<Arc<dyn OperationType>> {
    REGISTRY.get(type_id).cloned()
}

pub fn all() -> impl Iterator<Item = Arc<dyn OperationType>> {
    REGISTRY.values().cloned()
}
```

### Implementing an Operation Type

Each operation type is a zero-sized struct that implements the trait:

```rust
// src/toolpath/facing.rs
pub struct FacingOperation;

impl OperationType for FacingOperation {
    fn type_id(&self) -> &'static str { "facing" }
    fn display_name(&self) -> &'static str { "Facing" }

    fn parameter_schema(&self) -> ParameterSchema {
        ParameterSchema {
            fields: vec![
                FieldSchema {
                    name: "stepover",
                    display: "Stepover",
                    field_type: FieldType::Float,
                    unit: Some("%"),
                    required: false,
                    default: Some(json!(0.7)),
                    min: Some(0.01),
                    max: Some(1.0),
                },
                FieldSchema {
                    name: "start_depth",
                    display: "Start Depth",
                    field_type: FieldType::Float,
                    unit: Some("mm"),
                    required: true,
                    default: Some(json!(0.0)),
                    min: None,
                    max: None,
                },
                FieldSchema {
                    name: "final_depth",
                    display: "Final Depth",
                    field_type: FieldType::Float,
                    unit: Some("mm"),
                    required: true,
                    default: None,
                    min: None,
                    max: Some(0.0),
                },
                // feed_rate, plunge_rate, spindle_speed, depth_per_pass
                // are common to all operations — handled by the base schema
            ],
        }
    }

    fn validate(&self, op: &Operation, geometry: &PartGeometry) -> Vec<ValidationIssue> {
        let mut issues = vec![];
        let tool_diameter = op.tool(geometry).map(|t| t.diameter).unwrap_or(0.0);
        if tool_diameter == 0.0 {
            issues.push(ValidationIssue::error("No tool selected"));
        }
        // No validation for aggressive feeds — trust the operator.
        issues
    }

    fn generate(&self, op: &Operation, geometry: &PartGeometry) -> Result<Vec<ToolpathSegment>, ToolpathError> {
        // ... facing algorithm (see 04-toolpath-algorithms.md)
    }
}
```

### DrillOperation: compile_nc in Practice

Drill is the primary operation that overrides `compile_nc`. It reads cycle parameters directly from `op` — it does not reconstruct them from the generated move sequence:

```rust
// src/toolpath/drill.rs
pub struct DrillOperation;

impl OperationType for DrillOperation {
    fn type_id(&self) -> &'static str { "drill" }

    fn generate(&self, op: &Operation, geometry: &PartGeometry)
        -> Result<Vec<ToolpathSegment>, ToolpathError>
    {
        // Always produces explicit rapid/linear moves — used for visualization
        // and as fallback when post-processor has no cycle support.
        // ...
    }

    fn compile_nc(&self, op: &Operation, caps: &PostProcessorCapabilities)
        -> Option<Vec<NCBlock>>
    {
        // Emit cycle blocks unless the user explicitly opted out (use_canned_cycle defaults to true).
        // Post-processor capability is checked below; if unsupported, caller falls back to explicit moves.
        if !op.use_canned_cycle { return None; }

        let cycle_type = match op.drill_type {
            DrillType::Simple => "drill",
            DrillType::Peck   => "peck_drill",
            DrillType::Spot   => "spot_drill",
            DrillType::Bore   => "bore",
            DrillType::Tap    => "tap",
        };

        // If the post-processor can't handle this cycle, return None —
        // the compiler falls back to generic explicit-move conversion.
        if !caps.supported_cycles.contains(cycle_type) { return None; }

        let mut blocks = vec![];

        // One CycleDefine with all parameters taken directly from op.
        // No pattern matching on generated moves needed.
        blocks.push(NCBlock {
            block_type: BlockType::CycleDefine,
            params: hashmap! {
                "cycle_type" => json!(cycle_type),
                "z"          => json!(op.final_depth),
                "r"          => json!(op.r_plane()),        // start_depth + clearance
                "f"          => json!(op.plunge_rate),
                "q"          => json!(op.peck_depth),       // None for non-peck types
                "pitch"      => json!(op.tap_pitch),        // None for non-tap types
            },
        });

        // One CycleCall per drill point
        for point in op.drill_points() {
            blocks.push(NCBlock {
                block_type: BlockType::CycleCall,
                params: hashmap! { "x" => json!(point.x), "y" => json!(point.y) },
            });
        }

        blocks.push(NCBlock { block_type: BlockType::CycleOff, params: hashmap!{} });

        Some(blocks)
    }
}
```

### NC Compiler Orchestration

The compiler calls `compile_nc` before the generic path. The operation type decides; the compiler just orchestrates:

```rust
// src/nc/compiler.rs

pub fn compile_operation(
    op: &Operation,
    toolpath: &Toolpath,
    caps: &PostProcessorCapabilities,
) -> Vec<NCBlock> {
    let mut blocks = vec![];

    // Preamble: tool change, spindle, coolant, rapid to clearance
    blocks.extend(compile_preamble(op));

    // Core: operation-specific path OR generic segment conversion
    let core = registry::get(&op.operation_type)
        .and_then(|plugin| plugin.compile_nc(op, caps))
        .unwrap_or_else(|| compile_toolpath_generic(op, toolpath));

    blocks.extend(core);

    // Postamble: rapid to clearance, spindle off if last op for this tool
    blocks.extend(compile_postamble(op));

    blocks
}

/// Generic fallback: convert ToolpathSegments to NCBlocks one-to-one.
/// Used for facing, profile, pocket, and any drill without cycle support.
fn compile_toolpath_generic(op: &Operation, toolpath: &Toolpath) -> Vec<NCBlock> {
    toolpath.segments.iter().map(|seg| match seg.move_type {
        MoveType::Rapid  => NCBlock { block_type: BlockType::Rapid,  params: coords(seg) },
        MoveType::Linear => NCBlock { block_type: BlockType::Linear, params: coords_feed(seg) },
        MoveType::ArcCw  => NCBlock { block_type: BlockType::ArcCw,  params: arc_params(seg) },
        MoveType::ArcCcw => NCBlock { block_type: BlockType::ArcCcw, params: arc_params(seg) },
    }).collect()
}
```

### Full Export Sequence

```
User clicks Export → selects "heidenhain"
        │
        ▼
get_capabilities("heidenhain")          ← loads Lua, reads M.supported_cycles
  → PostProcessorCapabilities {
        supported_cycles: ["drill", "peck_drill", "tap", ...],
        skip_strategy: "block_delete",
    }
        │
        ▼
compile_program(operations, caps)
  → for each operation:
      plugin.compile_nc(op, caps)       ← DrillOperation: returns CycleDefine/CycleCall
                                           FacingOperation: returns None
      OR compile_toolpath_generic(...)  ← fallback for None
        │
        ▼
generate_nc_code(blocks, "heidenhain")  ← fresh Lua VM
  → M.generate(blocks, context)
  → format_cycl_def() for CycleDefine
  → "L X+n Y+n FMAX M99" for CycleCall
        │
        ▼
NC string → response to client
```

### How the API Uses the Registry

When the client sends `POST /api/project/operations` with `"type": "pocket"`:

1. API handler calls `registry::get("pocket")` to retrieve the type
2. Type's `parameter_schema()` validates the incoming fields
3. Operation is stored with `operation_type = "pocket"`

When `POST /api/project/operations/{id}/generate` is called (toolpath generation — separate from NC export):

```rust
async fn generate_toolpath(op_id: Uuid, state: AppState) -> Result<Response> {
    let op = state.project.read().get_operation(op_id)?;
    let geometry = state.project.read().get_geometry(op.geometry_id)?;
    let plugin = registry::get(&op.operation_type)
        .ok_or(Error::UnknownOperationType(op.operation_type.clone()))?;

    // spawn_blocking: OCCT calls and heavy geometry are not async-safe
    let segments = tokio::task::spawn_blocking(move || {
        plugin.generate(&op, &geometry)    // always explicit moves
    }).await??;

    state.project.write().set_toolpath(op_id, segments);
    state.ws.broadcast_toolpath_complete(op_id);
    Ok(StatusCode::OK.into_response())
}
```

NC export (`POST /api/project/export`) runs `compile_program` → `generate_nc_code` separately, after toolpaths are generated. The stored `Toolpath` is the explicit-move fallback; `compile_nc` may bypass it entirely for cycle-capable post-processors.

### Adding a New Operation Type

1. Create `src/toolpath/my_operation.rs`
2. Implement `OperationType` for a new zero-sized struct
3. Add it to `REGISTRY` in `registry.rs`
4. Add the type ID string to the `OperationType` enum in `core/operation.rs`
5. Add integration tests in `tests/`

The API, frontend property panel, and NC compiler all derive their behavior from the registry — no other files need to change.

### Validation Philosophy

`validate()` only returns errors for **physically impossible** situations:
- Tool diameter larger than the narrowest point in the pocket (tool can't fit)
- `final_depth` shallower than `start_depth`
- No geometry selected

It never warns about:
- Aggressive feed rates or spindle speeds
- Deep depth-per-pass values
- Unconventional strategies

Trust the operator. Only block what is geometrically impossible.
