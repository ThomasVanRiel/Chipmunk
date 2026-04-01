use crate::nc::ir::NCBlock;

use super::pattern::Pattern;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoveType {
    Rapid,
    Linear,
    ArcCw,
    ArcCcw,
    Pattern,
}

#[derive(Debug, Clone)]
pub struct ToolpathSegment {
    pub move_type: MoveType,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub comment: Option<String>,
    pub pattern: Option<NCBlock>,
}

impl ToolpathSegment {
    fn new(move_type: MoveType, x: f64, y: f64, z: f64) -> Self {
        ToolpathSegment {
            move_type,
            x,
            y,
            z,
            comment: None,
            pattern: None,
        }
    }

    pub fn rapid(x: f64, y: f64, z: f64) -> Self {
        Self::new(MoveType::Rapid, x, y, z)
    }

    pub fn linear(x: f64, y: f64, z: f64) -> Self {
        Self::new(MoveType::Linear, x, y, z)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Locations {
    Points { points: Vec<[f64; 2]> },
    Pattern { pattern: Pattern },
}
