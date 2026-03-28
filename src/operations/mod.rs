pub mod quill;
use crate::{
    core::{postprocessors::PostprocessorCapabilities, tool::Tool, toolpath::ToolpathSegment},
    nc::ir::NCBlock,
};
use anyhow::Result;
use quill::Quill;

pub struct Operation<'a> {
    pub common: OperationCommon<'a>,
    pub kind: OperationKind,
}

pub struct OperationCommon<'a> {
    pub name: String,
    pub tool: Tool,
    pub capabilities: &'a PostprocessorCapabilities,
    pub clearance: f64,
}

pub enum OperationKind {
    Quill(Quill),
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
            OperationKind::Quill(q) => q,
        }
    }

    pub fn generate(&self) -> Result<Vec<ToolpathSegment>> {
        self.kind_impl().generate(&self.common)
    }

    pub fn compile(&self, segments: &[ToolpathSegment]) -> Result<Vec<NCBlock>> {
        self.kind_impl().compile(&self.common, segments)
    }
}
