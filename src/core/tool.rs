use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpindleDirection {
    Cw,
    Ccw,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tool {
    pub tool_number: u32,
    pub name: String,
    pub diameter: f64,
    pub spindle_speed: f64,
    pub spindle_direction: SpindleDirection,
}

impl Default for Tool {
    fn default() -> Tool {
        Tool {
            tool_number: 1,
            name: "Unnamed Tool".to_string(),
            diameter: 0.0,
            spindle_speed: 400.0,
            spindle_direction: SpindleDirection::Cw,
        }
    }
}
