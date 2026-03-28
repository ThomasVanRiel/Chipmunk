use crate::toolpath::patterns::Pattern;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoveType {
    Rapid,
    Linear,
    ArcCw,
    ArcCcw,
}

#[derive(Debug, Clone)]
pub struct ToolpathSegment {
    pub move_type: MoveType,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Locations {
    Points { points: Vec<[f64; 2]> },
    Pattern { pattern: Pattern },
}
