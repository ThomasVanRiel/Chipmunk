use crate::core::postprocessors::PostprocessorCapabilities;
use crate::core::tool::Tool;
use crate::toolpath::patterns::Pattern;
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
