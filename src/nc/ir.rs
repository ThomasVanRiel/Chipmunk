use anyhow::Result;
use serde::Serialize;

use crate::core::tool::SpindleDirection;

#[derive(Debug, Clone, Serialize)]
pub struct NCState {
    spindle_on: bool,
    spindle_direction: SpindleDirection,
    coolant_on: bool,
}

impl Default for NCState {
    fn default() -> Self {
        NCState {
            spindle_on: false,
            spindle_direction: SpindleDirection::Cw,
            coolant_on: false,
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
                    state.spindle_on = true;
                    state.spindle_direction = *direction;
                }
                NCBlock::SpindleOff => {
                    state.spindle_on = false;
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
