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
    lua.load(base_lua).set_name("base").exec()?;
    let pp: LuaTable = lua.load(postprocessor_lua).eval()?;

    // Create the context table
    let context = lua.create_table()?;
    context.set("program_name", program_name)?;
    context.set("units", units)?;

    // Convert the blocks to lua table
    let blocks_table: Vec<LuaTable> = blocks
        .iter()
        .map(|block| block_to_lua(&lua, block))
        .collect::<LuaResult<Vec<LuaTable>>>()?;

    // Call the generate function of the postprocessor to return the NC program
    let generate_function = pp.get::<LuaFunction>("generate")?;
    let result = generate_function.call::<String>((blocks_table, context))?;
    Ok(result)
}

fn block_to_lua(lua: &Lua, block: &NCBlock) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    match block {
        NCBlock::ProgramStart { name, units } => {
            table.set("type", "program_start")?;
            table.set("name", name.as_str())?;
            table.set("units", units.as_str())?;
        }
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
        NCBlock::ProgramEnd { name, units } => {
            table.set("type", "program_end")?;
            table.set("name", name.as_str())?;
            table.set("units", units.as_str())?;
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
