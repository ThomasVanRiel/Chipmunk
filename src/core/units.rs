use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum Units {
    Mm,
    Inch,
}
