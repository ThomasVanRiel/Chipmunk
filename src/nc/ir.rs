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
