use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JobConfig {
    pub postprocessor: String,
    pub clearance: f64,
    pub operations: Vec<OperationConfig>,
}

#[derive(Debug, Deserialize)]
pub struct OperationConfig {
    #[serde(rename = "type")]
    pub operation_type: String,
    pub strategy: String,
    pub tool_number: Option<u32>,
    pub tool_name: Option<String>,
    pub tool_diameter: Option<f64>,
    pub spindle_speed: f64,
    pub points: Vec<[f64;2]>,
}

pub fn load_job(path: &str) -> anyhow::Result<JobConfig> {
    let contents = std::fs::read_to_string(path)?;
    let config: JobConfig = serde_yaml::from_str(&contents)?;
    Ok(config)
}
