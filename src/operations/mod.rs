pub mod drill;
pub mod quill;
use crate::{
    core::{postprocessors::PostprocessorCapabilities, tool::Tool, toolpath::ToolpathSegment},
    nc::ir::NCBlock,
};
use anyhow::Result;
use drill::Drill;
use quill::Quill;

pub struct Operation<'a> {
    pub common: OperationCommon<'a>,
    pub kind: OperationVariant,
}

pub struct OperationCommon<'a> {
    pub name: String,
    pub tool: Tool,
    pub capabilities: &'a PostprocessorCapabilities,
    pub clearance: f64,
}

pub enum OperationVariant {
    Quill(Quill),
    Drill(Drill),
}

pub trait OperationType {
    fn generate(&self, common: &OperationCommon) -> Result<Vec<ToolpathSegment>>;
    fn compile(
        &self,
        common: &OperationCommon,
        segments: &[ToolpathSegment],
    ) -> Result<Vec<NCBlock>>;
}

impl<'a> Operation<'a> {
    fn kind_impl(&self) -> &dyn OperationType {
        match &self.kind {
            OperationVariant::Quill(o) => o,
            OperationVariant::Drill(o) => o,
        }
    }

    pub fn generate(&self) -> Result<Vec<ToolpathSegment>> {
        self.kind_impl().generate(&self.common)
    }

    pub fn compile(&self, segments: &[ToolpathSegment]) -> Result<Vec<NCBlock>> {
        self.kind_impl().compile(&self.common, segments)
    }
}
