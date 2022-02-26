use super::{
    share_amount::ShareAmount,
    shares_specs::SharesRoundingMode::{self, Ceil, Floor},
};
use crate::decimal_util::AsDecimal;
use anyhow::{anyhow, Result};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use std::convert::TryFrom;

// A percentage in range [0..1]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharesPercentage(Decimal);

impl TryFrom<Decimal> for SharesPercentage {
    type Error = anyhow::Error;
    fn try_from(value: Decimal) -> Result<Self, Self::Error> {
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

    pub fn apply_to(
        &self,
        shares_supply: ShareAmount,
        rounding: SharesRoundingMode,
    ) -> Result<u64> {
        let d = self.0 * shares_supply.0.as_decimal();
        let res = match rounding {
            Floor => d.floor(),
            Ceil => d.ceil(),
        };
        // This should be safe to unwrap but being extra careful as with WASM unwrap failure is not traceable.
        res.to_u64().ok_or(anyhow!("Invalid state: floor/ceil should be always convertible to u64. self: {self:?}, rounding: {rounding:?}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::flows::create_project::shares_percentage::SharesPercentage;
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
        let investor_percentage: Decimal = "0.31231321".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    fn is_created_with_small_value_higher_than_0() -> Result<()> {
        let investor_percentage: Decimal = "0.000000000001".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    fn is_created_with_value_slightly_lower_than_1() -> Result<()> {
        let investor_percentage: Decimal = "0.999999999999".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_ok());

        Ok(())
    }
}
