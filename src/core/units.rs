use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum Units {
    #[default]
    Mm,
    Inch,
}
