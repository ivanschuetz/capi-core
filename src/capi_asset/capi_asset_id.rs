use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapiAssetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapiAssetAmount(pub u64);

impl Display for CapiAssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
