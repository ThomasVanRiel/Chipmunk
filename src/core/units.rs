use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum Units {
    #[default]
    Mm,
    Inch,
}

impl std::fmt::Display for Units {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Units::Mm => write!(f, "mm"),
            Units::Inch => write!(f, "inch"),
        }
    }
}
