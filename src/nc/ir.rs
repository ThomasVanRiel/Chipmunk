use anyhow::Result;
use serde::Serialize;

use crate::core::tool::SpindleState;

#[derive(Debug, Clone, Serialize)]
pub struct NCState {
    spindle: SpindleState,
    coolant: bool,
}

impl Default for NCState {
    fn default() -> Self {
        NCState {
            spindle: SpindleState::Off,
            coolant: false,
        }
    }
}
// NCBlocks are serializeable to lua tables using `lua.to_value(block)`
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NCBlock {
    OperationStart {
        text: Option<String>,
    },
    OperationEnd {
        text: Option<String>,
    },
    ToolChange {
        tool_number: Option<u32>,
        spindle_speed: f64,
    },
    Comment {
        text: String,
    },
    Stop,
    SpindleOn {
        direction: SpindleState,
    },
    SpindleOff,
    CoolantOn,
    CoolantOff,
    Retract {
        height: f64,
    },
    RetractFull,
    Home,
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

    // Canned Cycles
    CycleCall {
        x: f64,
        y: f64,
        z: f64,
    },
    CycleDrill {
        depth: f64,
        surface_position: f64,
        plunge_depth: f64,
        feed: f64,
        dwell_top: f64,
        dwell_bottom: f64,
        clearance: f64,
        second_clearance: f64,
        tip_trough: bool,
    },
}

#[derive(Debug, Serialize)]
pub struct AnnotatedBlock<'a> {
    pub block: &'a NCBlock,
    pub state: NCState,
}

pub fn annotate_blocks<'a>(blocks: &'a [NCBlock]) -> Result<Vec<AnnotatedBlock<'a>>> {
    let mut state = NCState::default();
    let annotated_blocks: Vec<AnnotatedBlock> = blocks
        .iter()
        .map(|block| {
            match block {
                NCBlock::SpindleOn { direction } => {
                    state.spindle = *direction;
                }
                NCBlock::SpindleOff => {
                    state.spindle = SpindleState::Off;
                }
                NCBlock::CoolantOn => {
                    state.coolant = true;
                }
                NCBlock::CoolantOff => {
                    state.coolant = false;
                }
                _ => {}
            }
            AnnotatedBlock {
                state: state.clone(),
                block,
            }
        })
        .collect();

    Ok(annotated_blocks)
}
