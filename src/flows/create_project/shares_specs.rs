use std::convert::TryInto;

use crate::decimal_util::AsDecimal;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::shares_percentage::SharesPercentage;

#[derive(Debug, Clone)]
pub enum SharesRoundingMode {
    Floor,
    Ceil,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharesDistributionSpecs {
    creator: u64,
    investors: u64,
}

impl SharesDistributionSpecs {
    /// Calculate the creator's and investor's part from the investor's % entered by the creator.
    pub fn from_investors_percentage(
        percentage: &SharesPercentage,
        shares_supply: u64,
    ) -> Result<SharesDistributionSpecs> {
        let creator_percentage: SharesPercentage =
            (1.as_decimal() - percentage.value()).try_into()?;
        // Creator's part is floored and investor's ceiled - we resolve fractionals in favor of the investors
        Self::new(
            creator_percentage.apply_to(shares_supply, SharesRoundingMode::Floor)?,
            percentage.apply_to(shares_supply, SharesRoundingMode::Ceil)?,
        )
    }

    pub fn new(creator: u64, investors: u64) -> Result<SharesDistributionSpecs> {
        creator.checked_add(investors).ok_or(anyhow!(
            "Creator shares: {creator} + investors shares: {investors} overflow"
        ))?;
        Ok(SharesDistributionSpecs { creator, investors })
    }

    pub fn investors(&self) -> u64 {
        self.investors
    }

    pub fn creator(&self) -> u64 {
        self.creator
    }
}

#[cfg(test)]
mod tests {
    use super::SharesDistributionSpecs;
    use crate::flows::create_project::shares_percentage::SharesPercentage;
    use anyhow::Result;
    use rust_decimal::Decimal;
    use std::convert::TryInto;

    #[test]
    fn test_shares_distribution_with_only_integers() -> Result<()> {
        let investor_percentage: Decimal = "0.3".parse().unwrap();
        let shares_investor_percentage: SharesPercentage = investor_percentage.try_into().unwrap();
        let supply = 100;

        let specs = SharesDistributionSpecs::from_investors_percentage(
            &shares_investor_percentage,
            supply,
        )?;

        assert_eq!(30, specs.investors());
        assert_eq!(70, specs.creator());

        Ok(())
    }

    #[test]
    fn test_shares_distribution_with_fractionals() -> Result<()> {
        let investor_percentage: Decimal = "0.33333333333333".parse().unwrap();
        let shares_investor_percentage: SharesPercentage = investor_percentage.try_into().unwrap();
        let supply = 100;

        let specs = SharesDistributionSpecs::from_investors_percentage(
            &shares_investor_percentage,
            supply,
        )?;

        // value (33.3333333333) always ceiled for the investors
        assert_eq!(34, specs.investors());
        // value (66.6666666666) always floored for the owner
        assert_eq!(66, specs.creator());

        Ok(())
    }

    #[test]
    fn test_shares_distribution_largest_number_and_fractionals() -> Result<()> {
        let investor_percentage: Decimal = "0.341".parse().unwrap();
        let shares_investor_percentage: SharesPercentage = investor_percentage.try_into().unwrap();
        let supply = u64::MAX;

        let specs = SharesDistributionSpecs::from_investors_percentage(
            &shares_investor_percentage,
            supply,
        )?;

        assert_eq!(6290339729134957101, specs.investors());
        assert_eq!(12156404344574594514, specs.creator());

        Ok(())
    }

    #[test]
    fn test_shares_distribution_random() -> Result<()> {
        let investor_percentage: Decimal = "0.45".parse().unwrap();
        let shares_investor_percentage: SharesPercentage = investor_percentage.try_into().unwrap();
        let supply = 12_234_234_234;

        let specs = SharesDistributionSpecs::from_investors_percentage(
            &shares_investor_percentage,
            supply,
        )?;

        assert_eq!(5505405406, specs.investors());
        assert_eq!(6728828828, specs.creator());

        Ok(())
    }

    #[test]
    fn test_shares_distribution_investors_close_to_1() -> Result<()> {
        let investor_percentage: Decimal = "0.999999999999".parse().unwrap();
        let shares_investor_percentage: SharesPercentage = investor_percentage.try_into().unwrap();
        let supply = 10_000;

        let specs = SharesDistributionSpecs::from_investors_percentage(
            &shares_investor_percentage,
            supply,
        )?;

        assert_eq!(10_000, specs.investors());
        assert_eq!(0, specs.creator());

        Ok(())
    }

    #[test]
    fn test_shares_distribution_investors_close_to_0() -> Result<()> {
        let investor_percentage: Decimal = "0.0000000001".parse().unwrap();
        let shares_investor_percentage: SharesPercentage = investor_percentage.try_into().unwrap();
        let supply = 10_000;

        let specs = SharesDistributionSpecs::from_investors_percentage(
            &shares_investor_percentage,
            supply,
        )?;

        assert_eq!(1, specs.investors());
        assert_eq!(9_999, specs.creator());

        Ok(())
    }

    #[test]
    fn test_shares_distribution_investors_0() -> Result<()> {
        let investor_percentage: Decimal = "0".parse().unwrap();
        let shares_investor_percentage: SharesPercentage = investor_percentage.try_into().unwrap();
        let supply = 10_000_000;

        let specs = SharesDistributionSpecs::from_investors_percentage(
            &shares_investor_percentage,
            supply,
        )?;

        assert_eq!(0, specs.investors());
        assert_eq!(10_000_000, specs.creator());

        Ok(())
    }

    #[test]
    fn test_shares_distribution_investors_1() -> Result<()> {
        let investor_percentage: Decimal = "1".parse().unwrap();
        let shares_investor_percentage: SharesPercentage = investor_percentage.try_into().unwrap();
        let supply = 10_000_000_000_123;

        let specs = SharesDistributionSpecs::from_investors_percentage(
            &shares_investor_percentage,
            supply,
        )?;

        assert_eq!(10_000_000_000_123, specs.investors());
        assert_eq!(0, specs.creator());

        Ok(())
    }

    #[test]
    fn test_shares_distribution_investors_more_than_1() -> Result<()> {
        let investor_percentage: Decimal = "1.1".parse().unwrap();
        let res: Result<SharesPercentage> = investor_percentage.try_into();
        assert!(res.is_err());

        Ok(())
    }
}
