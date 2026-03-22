use super::ir::NCBlock;
use crate::core::tool::Tool;
use crate::core::toolpath::ToolpathSegment;

pub fn compile_manual_drill(
    program_name: &str,
    units: &str,
    tool: &Tool,
    clearance_z: f64,
    segments: &[ToolpathSegment],
) -> Vec<NCBlock> {
    let mut blocks: Vec<NCBlock> = vec![
        NCBlock::ProgramStart {
            name: String::from(program_name),
            units: String::from(units),
        },
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
        NCBlock::Rapid {
            x: None,
            y: None,
            z: Some(clearance_z),
        },
    ];
    for segment in segments {
        blocks.push(NCBlock::Rapid {
            x: Some(segment.x),
            y: Some(segment.y),
            z: Some(clearance_z),
        });
    }
    blocks.push(NCBlock::SpindleOff);
    blocks.push(NCBlock::ProgramEnd {
        name: String::from(program_name),
    });
    blocks
}
