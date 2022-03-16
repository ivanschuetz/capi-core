use std::cmp::PartialOrd;
use std::fmt::Display;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::decimal_util::AsDecimal;

/// An amount of assets (ASA)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct AssetAmount(pub u64);

impl AssetAmount {
    pub fn as_decimal(&self) -> Decimal {
        self.0.as_decimal()
    }
}

impl Display for AssetAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq<u64> for AssetAmount {
    fn eq(&self, other: &u64) -> bool {
        &self.0 == other
    }
}

impl PartialOrd<u64> for AssetAmount {
    fn partial_cmp(&self, other: &u64) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}
