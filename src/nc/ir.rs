use crate::core::tool::SpindleDirection;

#[derive(Debug, Clone)]
pub enum NCBlock {
    ProgramStart {
        name: String,
        units: String,
    },
    ToolChange {
        tool_number: u32,
        spindle_speed: f64,
    },
    Comment {
        text: String,
    },
    Stop,
    SpindleOn {
        direction: SpindleDirection,
    },
    Rapid {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    SpindleOff,
    ProgramEnd {
        name: String,
    },
}
