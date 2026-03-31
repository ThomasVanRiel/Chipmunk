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
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Locations {
    Points { points: Vec<[f64; 2]> },
    Pattern { pattern: Pattern },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Pattern {
    Circular {
        cc: [f64; 2],
        diameter: Option<f64>,
        radius: Option<f64>,
        angle_start: Option<f64>,
        angle_stop: Option<f64>,
        angle_step: Option<f64>, // Nevative step means we step in other direction
        count: Option<i32>,      // Negative count means we step in other direction
    },
}

impl Pattern {
    // Pattern expansion can fail! e.g. diameter OR radius must be given (or both if they match)
    pub fn into_points(&self) -> Result<Vec<[f64; 2]>> {
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
                let r = match (diameter, radius) {
                    (Some(d), None) => d / 2.0,
                    (None, Some(r)) => *r,
                    (Some(d), Some(r)) => {
                        if (d / 2.0 - r).abs() > 1e-9 {
                            return Err(anyhow!(
                                "Circular pattern: diameter and radius are inconsistent"
                            ));
                        }
                        *r
                    }
                    (None, None) => {
                        return Err(anyhow!("Circular pattern: must specify diameter or radius"));
                    }
                };

                let a_start = angle_start.unwrap_or(0.0_f64).to_radians();
                let a_stop = angle_stop.unwrap_or(360.0_f64).to_radians();

                let angles: Vec<f64> = match (count, angle_step) {
                    (Some(_), Some(_)) => {
                        return Err(anyhow!(
                            "Circular pattern: specify count or angle_step, not both"
                        ));
                    }
                    (Some(n), None) => {
                        if *n == 0 {
                            return Err(anyhow!("Circular pattern: count must not be zero"));
                        }
                        let step = (a_stop - a_start) / (*n as f64);
                        (0..n.abs()).map(|i| a_start + i as f64 * step).collect()
                    }
                    (None, Some(step_deg)) => {
                        if *step_deg == 0.0 {
                            return Err(anyhow!("Circular pattern: angle_step must not be zero"));
                        }
                        let step = step_deg.to_radians();
                        let mut angles = vec![];
                        let mut a = a_start;
                        while (step > 0.0 && a <= a_stop + 1e-9)
                            || (step < 0.0 && a >= a_stop - 1e-9)
                        {
                            angles.push(a);
                            a += step;
                        }
                        angles
                    }
                    (None, None) => {
                        return Err(anyhow!(
                            "Circular pattern: must specify count or angle_step"
                        ));
                    }
                };

                Ok(angles
                    .iter()
                    .map(|&a| [cc[0] + r * a.cos(), cc[1] + r * a.sin()])
                    .collect())
            }
        }
    }
}
