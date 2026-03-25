use serde::Serialize;

use crate::core::tool::SpindleDirection;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NCBlock {
    ToolChange {
        tool_number: Option<u32>,
        spindle_speed: f64,
    },
    Comment {
        text: String,
    },
    Stop,
    SpindleOn {
        direction: SpindleDirection,
    },
    Retract {
        height: f64,
    },
    RetractFull,
    Rapid {
        x: f64,
        y: f64,
        z: f64,
    },
    Linear {
        x: f64,
        y: f64,
        z: f64,
        feed: f64,
    },
    SpindleOff,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::prelude::*;

    fn to_lua_table(lua: &Lua, block: &NCBlock) -> LuaResult<LuaTable> {
        let value = lua.to_value(block)?;
        match value {
            LuaValue::Table(t) => Ok(t),
            other => Err(LuaError::RuntimeError(format!(
                "expected table, got {}",
                other.type_name()
            ))),
        }
    }

    #[test]
    fn test_tool_change() {
        let lua = Lua::new();
        let block = NCBlock::ToolChange {
            tool_number: Some(3),
            spindle_speed: 12000.0,
        };
        let table = to_lua_table(&lua, &block).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "tool_change");
        assert_eq!(table.get::<u32>("tool_number").unwrap(), 3);
        assert_eq!(table.get::<f64>("spindle_speed").unwrap(), 12000.0);
    }

    #[test]
    fn test_tool_change_no_number() {
        let lua = Lua::new();
        let block = NCBlock::ToolChange {
            tool_number: None,
            spindle_speed: 8000.0,
        };
        let table = to_lua_table(&lua, &block).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "tool_change");
        assert!(matches!(
            table.get::<LuaValue>("tool_number").unwrap(),
            LuaValue::Nil | LuaValue::LightUserData(_)
        ));
        assert_eq!(table.get::<f64>("spindle_speed").unwrap(), 8000.0);
    }

    #[test]
    fn test_comment() {
        let lua = Lua::new();
        let block = NCBlock::Comment {
            text: "drill cycle".to_string(),
        };
        let table = to_lua_table(&lua, &block).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "comment");
        assert_eq!(table.get::<String>("text").unwrap(), "drill cycle");
    }

    #[test]
    fn test_stop() {
        let lua = Lua::new();
        let table = to_lua_table(&lua, &NCBlock::Stop).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "stop");
    }

    #[test]
    fn test_spindle_on_cw() {
        let lua = Lua::new();
        let block = NCBlock::SpindleOn {
            direction: SpindleDirection::Cw,
        };
        let table = to_lua_table(&lua, &block).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "spindle_on");
        assert_eq!(table.get::<String>("direction").unwrap(), "cw");
    }

    #[test]
    fn test_spindle_on_ccw() {
        let lua = Lua::new();
        let block = NCBlock::SpindleOn {
            direction: SpindleDirection::Ccw,
        };
        let table = to_lua_table(&lua, &block).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "spindle_on");
        assert_eq!(table.get::<String>("direction").unwrap(), "ccw");
    }

    #[test]
    fn test_spindle_off() {
        let lua = Lua::new();
        let table = to_lua_table(&lua, &NCBlock::SpindleOff).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "spindle_off");
    }

    #[test]
    fn test_retract() {
        let lua = Lua::new();
        let block = NCBlock::Retract { height: 50.0 };
        let table = to_lua_table(&lua, &block).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "retract");
        assert_eq!(table.get::<f64>("height").unwrap(), 50.0);
    }

    #[test]
    fn test_retract_full() {
        let lua = Lua::new();
        let table = to_lua_table(&lua, &NCBlock::RetractFull).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "retract_full");
    }

    #[test]
    fn test_rapid() {
        let lua = Lua::new();
        let block = NCBlock::Rapid {
            x: 10.0,
            y: 20.0,
            z: 5.0,
        };
        let table = to_lua_table(&lua, &block).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "rapid");
        assert_eq!(table.get::<f64>("x").unwrap(), 10.0);
        assert_eq!(table.get::<f64>("y").unwrap(), 20.0);
        assert_eq!(table.get::<f64>("z").unwrap(), 5.0);
    }

    #[test]
    fn test_linear() {
        let lua = Lua::new();
        let block = NCBlock::Linear {
            x: 100.0,
            y: 200.0,
            z: -5.0,
            feed: 250.0,
        };
        let table = to_lua_table(&lua, &block).unwrap();
        assert_eq!(table.get::<String>("type").unwrap(), "linear");
        assert_eq!(table.get::<f64>("x").unwrap(), 100.0);
        assert_eq!(table.get::<f64>("y").unwrap(), 200.0);
        assert_eq!(table.get::<f64>("z").unwrap(), -5.0);
        assert_eq!(table.get::<f64>("feed").unwrap(), 250.0);
    }
}
