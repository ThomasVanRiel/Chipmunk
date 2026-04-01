use crate::{
    core::{
        pattern::Pattern,
        toolpath::{Locations, ToolpathSegment},
    },
    nc::ir::NCBlock,
    operations::{OperationCommon, OperationType},
};
use anyhow::{Result, anyhow};

pub struct Drill {
    pub locations: Locations,
}

impl OperationType for Drill {
    fn generate(&self, common: &OperationCommon) -> Result<Vec<ToolpathSegment>> {
        // TODO: We need to check if we need to expand the drilling cycle or if the
        // postprocessor supports the canned cycle.
        match &self.locations {
            Locations::Points { points } => Ok(points
                .iter()
                .map(|[x, y]| ToolpathSegment::rapid(*x, *y, common.clearance))
                .collect::<Vec<_>>()),
            Locations::Pattern { pattern } => {
                // TODO: Write the translation dict "drill" <--> Drill into consts?
                // How can we do this in rust? Like serde::serialize?
                if let Some(cycle) = common.capabilities.cycles.get("drill") {
                    match pattern {
                        Pattern::Circular { .. } => {
                            // Check if the pattern is supported by the postprocessor
                            if cycle.iter().any(|p| p == "circular") {
                                Ok(vec![pattern.into_segment(common)?])
                            }
                            // Expand into points if it is unsupported
                            else {
                                Ok(pattern
                                    .into_points()?
                                    .iter()
                                    .map(|[x, y, z]| ToolpathSegment::rapid(*x, *y, *z))
                                    .collect::<Vec<_>>())
                            }
                        }
                        _ => Err(anyhow!(
                            "Drilling pattern {:?} not implemented yet!",
                            pattern
                        )),
                    }
                } else {
                    // TODO: Drilling cycle not supported to post, expand the pattern to points and
                    // generate drilling ToolpathSegments at each point.
                    Err(anyhow!(
                        "Drilling pattern {:?} not implemented yet!",
                        pattern
                    ))
                }
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
            NCBlock::CoolantOn,
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
                if let Some(text) = &segment.comment {
                    blocks.push(NCBlock::Comment { text: text.clone() });
                }
                if let Some(pattern) = &segment.pattern {
                    blocks.push(pattern.to_owned())
                } else {
                    blocks.push(NCBlock::CycleCall {
                        x: segment.x,
                        y: segment.y,
                        z: common.clearance,
                    });
                }
            }
        } else {
            // TODO: Calculate tool paths based on the segments.
            // Rapid to clearance, linear blocks plunge_depth down.
            // Actually, we need to know the capabilities already during generation,
            // where tool paths segments are generated. (At least in milling ...)
        };
        blocks.push(NCBlock::SpindleOff);
        blocks.push(NCBlock::CoolantOff);
        blocks.push(NCBlock::Retract {
            height: common.clearance,
        });
        blocks.push(NCBlock::OperationEnd { text: None });
        // SpindleOff optional, program end automatically stops spindle.
        // blocks.push(NCBlock::SpindleOff);
        Ok(blocks)
    }
}
