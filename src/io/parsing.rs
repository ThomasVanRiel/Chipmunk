
use crate::core::postprocessors::PostprocessorCapabilities;
use crate::core::tool::Tool;
use crate::core::toolpath::Locations;
use crate::core::units::Units;
use crate::operations::{OperationCommon, OperationVariant};
use crate::operations::{drill::Drill, quill::Quill};
use serde::Deserialize;
use anyhow::Result;

#[derive(Debug, Deserialize)]
pub struct JobConfig {
    pub name: Option<String>,
    pub postprocessor: String,
    pub clearance: f64,
    pub operations: Vec<OperationConfig>,
    #[serde(default)]
    pub units: Units,
}

#[derive(Debug, Deserialize)]
pub struct CommonOperationConfig {
    pub name: Option<String>,
    pub tool_number: Option<u32>,
    pub tool_name: Option<String>,
    pub tool_diameter: Option<f64>,
    pub spindle_speed: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationConfig {
    Quill {
        #[serde(flatten)]
        common: CommonOperationConfig,
        #[serde(flatten)]
        locations: Locations,
    },
    Drill {
        #[serde(flatten)]
        common: CommonOperationConfig,
        #[serde(flatten)]
        locations: Locations,
    }, // Milling {
       //     #[serde(flatten)]
       //     common: CommonOperationConfig,
       //     strategy: String,
       // },
}

impl OperationConfig {
    pub fn into_operation<'a>(
        self,
        clearance: f64,
        capabilities: &'a PostprocessorCapabilities,
    ) -> Result<crate::operations::Operation<'a>> {
        let (common_cfg, kind) = match self {
            OperationConfig::Quill { common, locations } => {
                (common, OperationVariant::Quill(Quill { locations }))
            }
            OperationConfig::Drill { common, locations } => {
                (common, OperationVariant::Drill(Drill { locations }))
            }
        };
        Ok(crate::operations::Operation {
            common: OperationCommon {
                name: common_cfg.name.unwrap_or_default(),
                // TODO: implement tool loading from config or library using
                // `common_cfg.into_tool()?`
                tool: Tool::default(),
                clearance,
                capabilities,
            },
            kind,
        })
    }
}
