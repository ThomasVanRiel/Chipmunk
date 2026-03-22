use crate::core::toolpath::{MoveType, ToolpathSegment};

pub fn manual_drill_toolpath(points: &[[f64; 2]], clearance_z: f64) -> Vec<ToolpathSegment> {
    // One rapid move per point, all at clearance height
    points
        .iter()
        .map(|&[x, y]| ToolpathSegment {
            move_type: MoveType::Rapid,
            x,
            y,
            z: clearance_z,
        })
        .collect()
}
