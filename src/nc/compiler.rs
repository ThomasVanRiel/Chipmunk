use super::ir::NCBlock;
use crate::core::tool::Tool;
use crate::core::toolpath::ToolpathSegment;

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
