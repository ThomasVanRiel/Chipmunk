use crate::core::tool::SpindleDirection;

#[derive(Debug, Clone)]
pub enum NCBlock {
    ProgramStart {name: String, units: String},
    ToolChange {tool_number: u32, spindle_speed: f64},
    Comment {text:String},
    Stop,
    SpindleOn {direction: SpindleDirection},
    Rapid {x:f64, y:f64,z:f64},
    SpindleOff,
    ProgramEnd {nam: String},
})
