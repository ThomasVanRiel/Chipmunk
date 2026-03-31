use super::ir::NCBlock;
use crate::{core::postprocessors::PostprocessorCapabilities, nc::ir::annotate_blocks};
use mlua::prelude::*;

const BASE_LUA: &str = include_str!("../../postprocessors/base.lua");

// Parse lua to get postprocessor cycle/pattern support
pub fn get_capabilities(postprocessor_lua: &str) -> anyhow::Result<PostprocessorCapabilities> {
    let lua = Lua::new();
    //
    // Register base.lua as a preloaded module so post-processors can require("base")
    let base_src = BASE_LUA.to_string();
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "base",
        lua.create_function(move |lua, ()| lua.load(&*base_src).eval::<LuaValue>())?,
    )?;

    let pp: LuaTable = lua.load(postprocessor_lua).eval()?;
    let capabilities: PostprocessorCapabilities =
        match pp.get::<Option<LuaValue>>("capabilities")? {
            Some(v) => lua.from_value(v)?,
            None => PostprocessorCapabilities::default(),
        };

    Ok(capabilities)
}

pub fn generate_nc(
    postprocessor_lua: &str,
    blocks: &[NCBlock],
    program_name: String,
    units: String,
) -> anyhow::Result<String> {
    // Create Lua VM
    let lua = Lua::new();
    let serialize_options = mlua::SerializeOptions::new().serialize_none_to_null(false);

    // Register base.lua as a preloaded module so post-processors can require("base")
    let base_src = BASE_LUA.to_string();
    let preload: LuaTable = lua.globals().get::<LuaTable>("package")?.get("preload")?;
    preload.set(
        "base",
        lua.create_function(move |lua, ()| lua.load(&*base_src).eval::<LuaValue>())?,
    )?;

    let pp: LuaTable = lua.load(postprocessor_lua).eval()?;

    // Create the context table
    let context = lua.create_table()?;
    context.set("name", program_name)?;
    context.set("units", units)?;

    // Convert the blocks to lua table
    let annotated_blocks = annotate_blocks(blocks)?;
    let blocks_table: Vec<LuaValue> = annotated_blocks
        .iter()
        .map(|block| {
            // Add the state key to the lua table here because serde does not handle a key named
            // "type" how we want during flattening.
            let block_val = lua.to_value_with(block.block, serialize_options)?;
            let state_val = lua.to_value_with(&block.state, serialize_options)?;
            if let (LuaValue::Table(block_table), _) = (&block_val, &state_val) {
                block_table.set("state", state_val)?;
            }
            Ok(block_val)
        })
        .collect::<LuaResult<Vec<LuaValue>>>()?;

    // Call the generate function of the postprocessor to return the NC program
    let generate_function = pp.get::<LuaFunction>("generate")?;
    let mut result: LuaMultiValue = generate_function.call((blocks_table, context))?;

    let first = result
        .pop_front()
        .ok_or_else(|| anyhow::anyhow!("Postprocessor returned no values"))?;

    // Extensive error handling because the postprocessor is external.
    // We support development as much as possible
    match first {
        LuaValue::String(s) => {
            // Success, postprocessor returned a string, we trust it is the full NC program
            let mut nc = s.to_str()?.to_string();
            // Add a newline to terminate the string if the postprocessor did not add one
            if !nc.ends_with("\n") {
                nc.push('\n');
            }
            Ok(nc)
        }
        LuaValue::Nil => {
            // Error: postprocessor returned `nil, str`
            let msg = result
                .pop_front()
                .and_then(|v| match v {
                    LuaValue::String(s) => s.to_str().ok().map(|s| s.to_string()),
                    _ => None,
                })
                .unwrap_or_else(|| "unknown postprocessor error".to_string());
            Err(anyhow::anyhow!("{}", msg))
        }
        other => Err(anyhow::anyhow!(
            "Postprocessor returned unexpected type: {}",
            other.type_name()
        )),
    }
}
