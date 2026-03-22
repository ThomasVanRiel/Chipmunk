use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
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
