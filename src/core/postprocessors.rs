use std::collections::HashMap;

#[derive(Debug, Default, serde::Deserialize)]
pub struct PostprocessorCapabilities {
    #[serde(default)]
    pub cycles: HashMap<String, Vec<String>>,
}
