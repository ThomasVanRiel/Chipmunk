use anyhow::{Result, anyhow};
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    Circular {
        cc: [f64; 2],
        // TODO: We need to check that only one of diameter/radius is specified!
        diameter: Option<f64>,
        radius: Option<f64>,
        angle_start: Option<f64>,
        angle_stop: Option<f64>,
        angle_step: Option<f64>,
        count: Option<u32>,
    },
}

impl Pattern {
    // Pattern expansion can fail! e.g. diameter OR radius must be given (or both if they match)
    #[allow(unused)] // TODO: 
    pub fn into_points(self) -> Result<Vec<[f64; 2]>> {
        match self {
            Pattern::Circular {
                cc,
                diameter,
                radius,
                angle_start,
                angle_stop,
                angle_step,
                count,
            } => {
                let p0 = cc;
                Err(anyhow!("Not implemented yet"))
            }
        }
    }
}
