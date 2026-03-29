use super::parsing::*;
use crate::nc::ir::NCBlock;
use crate::nc::{self, bridge};
use anyhow::anyhow;
use std::path::Path;

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

pub fn run_job(job: JobConfig) -> anyhow::Result<String> {
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

    // Verify at least one operation is defined
    if job.operations.is_empty() {
        return Err(anyhow!("No operations defined"));
    };
    // Iterate over all operations
    let blocks: Vec<NCBlock> = job
        .operations
        .into_iter() // TODO: Implement parallelization here using `par_iter` from rayon?
        .map(|config| -> anyhow::Result<Vec<NCBlock>> {
            let op = config.into_operation(job.clearance, capabilities)?;
            let segments = op.generate()?;
            op.compile(&segments)
        })
        .collect::<anyhow::Result<Vec<Vec<NCBlock>>>>()?
        .into_iter()
        .flatten()
        .collect();
    // TODO: Update program name and units
    nc::bridge::generate_nc(
        &pp_lua,
        &blocks,
        job.name.clone().unwrap_or("unnamed".to_string()),
        format!("{}", job.units),
    )
}
