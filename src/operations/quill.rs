use crate::{
    core::toolpath::{Locations, MoveType, ToolpathSegment},
    nc::ir::NCBlock,
    operations::{OperationCommon, OperationType},
};
use anyhow::{Result, anyhow};

pub struct Quill {
    pub locations: Locations,
}

impl OperationType for Quill {
    fn generate(&self, common: &OperationCommon) -> Result<Vec<ToolpathSegment>> {
        match &self.locations {
            Locations::Points { points } => Ok(points
                .iter()
                .map(|[x, y]| ToolpathSegment {
                    move_type: MoveType::Rapid,
                    x: *x,
                    y: *y,
                    z: common.clearance,
                })
                .collect::<Vec<_>>()),
            Locations::Pattern { pattern } => Err(anyhow!(
                "Drilling pattern {:?} not implemented yet!",
                pattern
            )),
        }
    }
    fn compile(
        &self,
        common: &OperationCommon,
        segments: &[ToolpathSegment],
    ) -> Result<Vec<NCBlock>> {
        let mut blocks: Vec<NCBlock> = vec![
            NCBlock::ToolChange {
                tool_number: Some(common.tool.tool_number),
                spindle_speed: common.tool.spindle_speed,
            },
            NCBlock::Comment {
                text: String::from("ENABLE SINGLE BLOCK MODE FOR QUILL DRILLING"),
            },
            NCBlock::Stop,
            NCBlock::SpindleOn {
                direction: common.tool.spindle_direction,
            },
            NCBlock::Retract {
                height: common.clearance,
            },
        ];
        for segment in segments {
            blocks.push(NCBlock::Rapid {
                x: segment.x,
                y: segment.y,
                z: common.clearance,
            });
        }
        blocks.push(NCBlock::Retract {
            height: common.clearance,
        });
        // SpindleOff optional, program end automatically stops spindle.
        // blocks.push(NCBlock::SpindleOff);
        Ok(blocks)
    }
}
