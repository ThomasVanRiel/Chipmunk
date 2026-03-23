use crate::core::tool::SpindleDirection;

use super::ir::NCBlock;
use mlua::prelude::*;

pub fn generate_nc(
    base_lua: &str,
    postprocessor_lua: &str,
    blocks: &[NCBlock],
    program_name: &str,
    units: &str,
) -> anyhow::Result<String> {
    // Create Lua VM
    let lua = Lua::new();

    // Load base and postprocessor into vm
    // TODO: require("base") might be better for pp development. Using global functions as is is
    // also possible, decide later.
    lua.load(base_lua).set_name("base").exec()?;
    let pp: LuaTable = lua.load(postprocessor_lua).eval()?;

    // Create the context table
    let context = lua.create_table()?;
    context.set("name", program_name)?;
    context.set("units", units)?;

    // Convert the blocks to lua table
    let blocks_table: Vec<LuaTable> = blocks
        .iter()
        .map(|block| block_to_lua(&lua, block))
        .collect::<LuaResult<Vec<LuaTable>>>()?;

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

fn block_to_lua(lua: &Lua, block: &NCBlock) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    match block {
        NCBlock::ToolChange {
            tool_number,
            spindle_speed,
        } => {
            table.set("type", "tool_change")?;
            table.set("tool_number", *tool_number)?;
            table.set("spindle_speed", *spindle_speed)?;
        }
        NCBlock::Comment { text } => {
            table.set("type", "comment")?;
            table.set("comment", text.as_str())?;
        }
        NCBlock::Stop => {
            table.set("type", "stop")?;
        }
        NCBlock::SpindleOn { direction } => {
            table.set("type", "spindle_on")?;
            match direction {
                SpindleDirection::Cw => {
                    table.set("direction", "cw")?;
                }
                SpindleDirection::Ccw => {
                    table.set("direction", "ccw")?;
                }
            }
        }
        NCBlock::SpindleOff => {
            table.set("type", "spindle_off")?;
        }
        NCBlock::Rapid { x, y, z } => {
            table.set("type", "rapid")?;
            table.set("x", *x)?;
            table.set("y", *y)?;
            table.set("z", *z)?;
        }
        #[allow(unreachable_patterns)]
        _ => {
            tracing::error!("Unsupported NCBlock: {:?}", block);
            return Err(LuaError::RuntimeError(format!(
                "Unsupported block type: {:?}",
                block
            )));
        }
    }
    Ok(table)
}
