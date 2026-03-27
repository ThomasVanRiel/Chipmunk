use crate::core::operation::{self, OperationLocations, OperationType, process_drilling};
use crate::core::tool::Tool;
use crate::core::units::Units;
use crate::io::job::operation::DrillStrategy;
use crate::nc::ir::NCBlock;
use crate::nc::{self, bridge};
use anyhow::anyhow;
use serde::Deserialize;
use std::path::Path;

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
    Drilling {
        #[serde(flatten)]
        common: CommonOperationConfig,
        strategy: DrillStrategy,
        #[serde(flatten)]
        locations: OperationLocations,
    },
    Milling {
        #[serde(flatten)]
        common: CommonOperationConfig,
        strategy: String,
    },
}

pub fn load_job(path: &str) -> anyhow::Result<JobConfig> {
    let contents = std::fs::read_to_string(path)?;
    let mut config: JobConfig = serde_yml::from_str(&contents)?;

    // If no name was given, we use the file stem as name
    config.name = config.name.or_else(|| {
        Some(
            Path::new(path)
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        )
    });

    // Return cleaned config
    Ok(config)
}

pub fn run_job(job: &JobConfig) -> anyhow::Result<String> {
    // Check if the postprocessor exists
    let pp_path = nc::postprocessors::find_postprocessor(&job.postprocessor)
        .ok_or(anyhow!("Postprocessor {} not found!", &job.postprocessor))?;

    // Load PP
    let pp_lua = std::fs::read_to_string(&pp_path).unwrap_or_else(|e| {
        eprintln!("Error reading post-processor: {}", e);
        std::process::exit(1);
    });

    // Get PP capabilities
    let capabilities = &bridge::get_capabilities(&pp_lua)?;

    let global_clearance = job.clearance;

    // Verify at least one operation is defined
    if job.operations.is_empty() {
        return Err(anyhow!("No operations defined"));
    };
    // Iterate over all operations
    let blocks: Vec<NCBlock> = job
        .operations
        .iter() // TODO: Implement parallelization here using `par_iter` from rayon?
        .map(|operation| -> anyhow::Result<Vec<NCBlock>> {
            match operation {
                OperationConfig::Drilling {
                    common,
                    strategy,
                    locations,
                } => {
                    tracing::info!("Processing drilling operation.");
                    let name = common
                        .name
                        .clone()
                        .unwrap_or("Unnamed Drilling Operation".to_string());

                    // TODO: Get default values from the tool registery if tool id is given.
                    // Extract tool processing as a function, which can handle tool libraries.
                    let tool = Tool {
                        tool_number: 1,
                        name: common.tool_name.clone().unwrap_or_default(),
                        diameter: common.tool_diameter.unwrap_or(0.0),
                        spindle_direction: crate::core::tool::SpindleDirection::Cw,
                        spindle_speed: common.spindle_speed.unwrap_or(800.0),
                    };
                    let op = operation::Operation {
                        global_clearance,
                        name,
                        operation_type: OperationType::Drilling,
                        tool,
                        capabilities,
                        locations,
                    };
                    process_drilling(&op, strategy)
                }
                op => Err(anyhow!("Operation {:?} not implemented yet", op)),
            }
        })
        .collect::<anyhow::Result<Vec<Vec<NCBlock>>>>()?
        .into_iter()
        .flatten()
        .collect();
    nc::bridge::generate_nc(&pp_lua, &blocks, "test", "mm")
}
