use crate::{asset_amount::AssetAmount, decimal_util::AsDecimal};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Sub},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundsAmount(pub AssetAmount);

impl Display for FundsAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl FundsAmount {
    pub fn new(amount: u64) -> FundsAmount {
        FundsAmount(AssetAmount(amount))
    }

    pub fn as_decimal(&self) -> Decimal {
        self.0 .0.as_decimal()
    }

    pub fn val(&self) -> u64 {
        self.0 .0
    }
}

// TODO use only checked operations!

impl Add for FundsAmount {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        FundsAmount::new(self.val() + rhs.val())
    }
}

impl Sub for FundsAmount {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        FundsAmount::new(self.val() - rhs.val())
    }
}

impl Mul for FundsAmount {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        FundsAmount::new(self.val() * rhs.val())
    }
}

impl Div for FundsAmount {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        FundsAmount::new(self.val() / rhs.val())
    }
}

impl Add<u64> for FundsAmount {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        FundsAmount::new(self.val() + rhs)
    }
}

impl Sub<u64> for FundsAmount {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        FundsAmount::new(self.val() - rhs)
    }
}

impl Mul<u64> for FundsAmount {
    type Output = Self;
    fn mul(self, rhs: u64) -> Self::Output {
        FundsAmount::new(self.val() * rhs)
    }
}

impl Div<u64> for FundsAmount {
    type Output = Self;
    fn div(self, rhs: u64) -> Self::Output {
        FundsAmount::new(self.val() / rhs)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundsAssetId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Funds {
    pub asset_id: FundsAssetId,
    pub amount: FundsAmount,
}
