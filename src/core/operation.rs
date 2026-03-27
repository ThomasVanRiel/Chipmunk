use crate::core::tool::Tool;
use crate::core::toolpath::{MoveType, ToolpathSegment};
use crate::nc::compiler::compile_manual_drill;
use crate::nc::ir::NCBlock;
use crate::nc::postprocessors::PostprocessorCapabilities;
use crate::toolpath::patterns::Pattern;
use anyhow::anyhow;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrillStrategy {
    Manual,
    General,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DrillParams {
    pub strategy: DrillStrategy,
    pub points: Vec<[f64; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OperationLocations {
    Points { points: Vec<[f64; 2]> },
    Pattern { pattern: Pattern },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Drilling,
    Milling,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            OperationType::Drilling => write!(f, "Drilling"),
            OperationType::Milling => write!(f, "Milling"),
        }
    }
}

#[derive(Debug)]
pub struct Operation<'a> {
    pub name: String,
    pub operation_type: OperationType,
    pub tool: Tool,
    pub global_clearance: f64,
    pub capabilities: &'a PostprocessorCapabilities,
    pub locations: &'a OperationLocations,
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
