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
