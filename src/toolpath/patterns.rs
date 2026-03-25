use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    List {
        points: Vec<[f64; 2]>,
    },
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
