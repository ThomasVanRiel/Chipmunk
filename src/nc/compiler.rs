use super::ir::NCBlock;
use crate::core::operation::{DrillStrategy, Operation, OperationLocations};
use crate::core::tool::Tool;
use crate::core::toolpath::{MoveType, ToolpathSegment};
use anyhow::anyhow;

pub fn compile_manual_drill(
    tool: &Tool,
    clearance_z: f64,
    segments: &[ToolpathSegment],
) -> Vec<NCBlock> {
    let mut blocks: Vec<NCBlock> = vec![
        NCBlock::ToolChange {
            tool_number: Some(tool.tool_number),
            spindle_speed: tool.spindle_speed,
        },
        NCBlock::Comment {
            text: String::from("ENABLE SINGLE BLOCK MODE FOR MANUAL QUILL DRILLING"),
        },
        NCBlock::Stop,
        NCBlock::SpindleOn {
            direction: tool.spindle_direction,
        },
        NCBlock::Retract {
            height: clearance_z,
        },
    ];
    for segment in segments {
        blocks.push(NCBlock::Rapid {
            x: segment.x,
            y: segment.y,
            z: clearance_z,
        });
    }
    blocks.push(NCBlock::Retract {
        height: clearance_z,
    });
    // SpindleOff optional, program end automatically stops spindle.
    // blocks.push(NCBlock::SpindleOff);
    blocks
}

pub fn process_drilling(
    operation: &Operation,
    strategy: &DrillStrategy,
) -> anyhow::Result<Vec<NCBlock>> {
    // TODO: We ignore patterns for now. A pattern copies the tool path and executes it with a
    // different offset by transforming the coordinates.
    match strategy {
        DrillStrategy::Manual => {
            let segments: Vec<ToolpathSegment> = match operation.locations {
                OperationLocations::Points { points } => Ok(points
                    .iter()
                    .map(|[x, y]| ToolpathSegment {
                        move_type: MoveType::Rapid,
                        x: *x,
                        y: *y,
                        z: operation.global_clearance,
                    })
                    .collect::<Vec<_>>()),
                OperationLocations::Pattern { pattern } => Err(anyhow!(
                    "Drilling pattern {:?} not implemented yet!",
                    pattern
                )),
            }?;
            Ok(compile_manual_drill(
                &operation.tool,
                operation.global_clearance,
                &segments,
            ))
        }
        DrillStrategy::General => Err(anyhow!("General drilling not implemented yet!")),
    }
}
