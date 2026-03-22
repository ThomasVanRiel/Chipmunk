use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub enum DrillStrategy {
    Manual,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DrillParams {
    pub strategy: DrillStrategy,
    pub points: Vec<[f64; 2]>,
}
