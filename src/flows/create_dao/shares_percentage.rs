use crate::decimal_util::AsDecimal;
use anyhow::{anyhow, Result};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

/// 4 decimals allow us to represent percentages with 2 decimals, e.g. 0.05%
/// and percentages with 2 decimals is fine for all our purposes
/// for capi fee, we likely need 1 decimal (e.g. to charge 0.5%)
/// for investor's share, integers are likely fine in most cases, so 0 decimals
/// the 2nd decimal is just in case
const MAX_DECIMALS: u32 = 4;

// A percentage in range [0..1]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharesPercentage(Decimal);

impl TryFrom<Decimal> for SharesPercentage {
    type Error = anyhow::Error;
    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
        if value.scale() > MAX_DECIMALS {
            return Err(anyhow!(
                "Shares percentage must not have more than {MAX_DECIMALS:?} decimals"
            ));
        }

        let min = 0.into();
        let max = 1.into();
        if value >= min && value <= max {
            Ok(SharesPercentage(value))
        } else {
            Err(anyhow!(
                "Invalid percentage value: {value}. Must be [{min}..{max}]"
            ))
        }
    }
}

impl SharesPercentage {
    pub fn value(&self) -> Decimal {
        self.0
    }

    /// u64 because that's what we use in most contexts
    pub fn to_u64(&self) -> Result<u64> {
        let multiplier = Self::conversion_integer_multiplier();
        Ok((self.0 * multiplier).to_u64().ok_or(anyhow!("Invalid state: since we allow max {MAX_DECIMALS} digits, multiplying by {multiplier} should yield an integer"))?)
    }

    fn conversion_integer_multiplier() -> Decimal {
        10u64.pow(MAX_DECIMALS).as_decimal()
    }
}

impl TryFrom<u64> for SharesPercentage {
    type Error = anyhow::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let multiplier = Self::conversion_integer_multiplier();
        let res = value.as_decimal().checked_div(multiplier).ok_or(anyhow!(
            "Unexpected: division failed: {value} / {multiplier}"
        ))?;
        Ok(SharesPercentage(res))
    }
}

#[cfg(test)]
mod tests {
    use crate::flows::create_dao::shares_percentage::SharesPercentage;
    use anyhow::Result;
    use rust_decimal::Decimal;
    use std::convert::TryInto;

    #[test]
    fn test_shares_error_when_created_with_larger_than_1() -> Result<()> {
        let investor_percentage: Decimal = "1.000000001".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        let investor_percentage: Decimal = "1.1".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        let investor_percentage: Decimal = "2".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn test_shares_error_when_created_with_less_than_0() -> Result<()> {
        let investor_percentage: Decimal = "-0.00000001".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        let investor_percentage: Decimal = "-1.1".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        let investor_percentage: Decimal = "-2".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn is_created_with_0() -> Result<()> {
        let investor_percentage: Decimal = "0".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    fn is_created_with_1() -> Result<()> {
        let investor_percentage: Decimal = "1".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    fn is_created_with_value_between_0_1() -> Result<()> {
        let investor_percentage: Decimal = "0.3123".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    fn is_created_with_small_value_higher_than_0() -> Result<()> {
        let investor_percentage: Decimal = "0.0001".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    fn is_created_with_value_slightly_lower_than_1() -> Result<()> {
        let investor_percentage: Decimal = "0.9999".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    fn creation_fails_with_more_than_allowed_decimals() -> Result<()> {
        let investor_percentage: Decimal = "0.12345".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn creation_fails_with_more_than_allowed_decimals_if_0() -> Result<()> {
        let investor_percentage: Decimal = "0.00000".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn creation_fails_with_more_than_allowed_decimals_if_0_and_1() -> Result<()> {
        let investor_percentage: Decimal = "0.00001".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        Ok(())
    }
}
