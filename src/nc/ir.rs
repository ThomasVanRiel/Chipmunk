use crate::core::tool::SpindleDirection;

// TODO: name and units do not belong in the IR blocks, they are context.
#[derive(Debug, Clone)]
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
    Rapid {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    SpindleOff,
}
