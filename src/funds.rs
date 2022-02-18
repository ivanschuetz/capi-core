use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Sub},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundsAmount(pub u64);

impl Display for FundsAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// TODO use only checked operations!

impl Add for FundsAmount {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        FundsAmount(self.0 + rhs.0)
    }
}

impl Sub for FundsAmount {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        FundsAmount(self.0 - rhs.0)
    }
}

impl Mul for FundsAmount {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        FundsAmount(self.0 * rhs.0)
    }
}

impl Div for FundsAmount {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        FundsAmount(self.0 / rhs.0)
    }
}

impl Add<u64> for FundsAmount {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        FundsAmount(self.0 + rhs)
    }
}

impl Sub<u64> for FundsAmount {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        FundsAmount(self.0 - rhs)
    }
}

impl Mul<u64> for FundsAmount {
    type Output = Self;
    fn mul(self, rhs: u64) -> Self::Output {
        FundsAmount(self.0 * rhs)
    }
}

impl Div<u64> for FundsAmount {
    type Output = Self;
    fn div(self, rhs: u64) -> Self::Output {
        FundsAmount(self.0 / rhs)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundsAssetId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Funds {
    pub asset_id: FundsAssetId,
    pub amount: FundsAmount,
}
