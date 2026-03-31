use crate::{
    core::toolpath::{Locations, MoveType, ToolpathSegment},
    nc::ir::NCBlock,
    operations::{OperationCommon, OperationType},
};
use anyhow::{Result, anyhow};

pub struct Drill {
    pub locations: Locations,
}

impl OperationType for Drill {
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
            Locations::Pattern { pattern } => {
                // TODO: For patterns, we need to check if the pattern is in the PP capabilities.
                // If it is not, we expand the pattern into points.
                Err(anyhow!(
                    "Drilling pattern {:?} not implemented yet!",
                    pattern
                ))
            }
        }
    }

    fn compile(
        &self,
        common: &OperationCommon,
        segments: &[ToolpathSegment],
    ) -> Result<Vec<NCBlock>> {
        let mut blocks: Vec<NCBlock> = vec![
            // TODO: Use the correct tool parameters
            NCBlock::ToolChange {
                tool_number: Some(common.tool.tool_number),
                spindle_speed: common.tool.spindle_speed,
            },
            NCBlock::OperationStart { text: None },
            NCBlock::SpindleOn {
                direction: common.tool.spindle_direction,
            },
            NCBlock::Retract {
                height: common.clearance,
            },
        ];
        // TODO: Check if CycleDrill is supported by the postprocessor
        // TODO: Populate fields from config
        if common.capabilities.cycles.contains_key("drill") {
            blocks.push(NCBlock::CycleDrill {
                depth: 20.0,
                surface_position: 0.0,
                plunge_depth: 0.0,
                feed: 100.0,
                dwell_top: 0.0,
                dwell_bottom: 0.0,
                clearance: 5.0,
                second_clearance: 20.0,
                tip_trough: false,
            });
            for segment in segments {
                blocks.push(NCBlock::CycleCall {
                    x: segment.x,
                    y: segment.y,
                    z: common.clearance,
                });
            }
        } else {
            // TODO: Calculate tool paths based on the segments.
            // Rapid to clearance, linear blocks plunge_depth down.
        };
        blocks.push(NCBlock::Retract {
            height: common.clearance,
        });
        blocks.push(NCBlock::OperationEnd { text: None });
        // SpindleOff optional, program end automatically stops spindle.
        // blocks.push(NCBlock::SpindleOff);
        Ok(blocks)
    }
}
