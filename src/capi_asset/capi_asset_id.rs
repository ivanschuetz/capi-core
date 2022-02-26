use crate::{asset_amount::AssetAmount, decimal_util::AsDecimal};
use rust_decimal::Decimal;
use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapiAssetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapiAssetAmount(pub AssetAmount);

impl CapiAssetAmount {
    pub fn new(amount: u64) -> CapiAssetAmount {
        CapiAssetAmount(AssetAmount(amount))
    }

    pub fn as_decimal(&self) -> Decimal {
        self.0 .0.as_decimal()
    }

    pub fn val(&self) -> u64 {
        self.0 .0
    }
}

impl Display for CapiAssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
