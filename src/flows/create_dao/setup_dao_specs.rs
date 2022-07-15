use super::model::CreateSharesSpecs;
use anyhow::{anyhow, Result};
use mbase::models::{
    funds::FundsAmount, share_amount::ShareAmount, shares_percentage::SharesPercentage,
    timestamp::Timestamp,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupDaoSpecs {
    pub name: String,
    pub descr_url: Option<String>,
    pub shares: CreateSharesSpecs,
    pub investors_share: SharesPercentage,
    pub share_price: FundsAmount,
    pub image_url: Option<String>,
    pub social_media_url: String, // this can be later in an extension (possibly with more links)
    // shares to be sold to investors (the rest stay in the creator's account)
    // note this is entirely different from investors_share, which is the % of the project's income channeled to investors
    shares_for_investors: ShareAmount,
    // we manage this as timestamp instead of date,
    // to ensure correctness when storing the timestamp in TEAL / compare to current TEAL timestamp (which is in seconds)
    // DateTime can have millis and nanoseconds too,
    // which would e.g. break equality comparisons between these specs and the ones loaded from global state
    pub raise_end_date: Timestamp,
    pub raise_min_target: FundsAmount,
}

impl SetupDaoSpecs {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        descr_url: Option<String>,
        shares: CreateSharesSpecs,
        investors_share: SharesPercentage,
        share_price: FundsAmount,
        image_url: Option<String>,
        social_media_url: String,
        shares_for_investors: ShareAmount,
        raise_min_target: FundsAmount,
        raise_end_date: Timestamp,
    ) -> Result<SetupDaoSpecs> {
        if shares_for_investors > shares.supply {
            return Err(anyhow!(
                "Shares for investors: {shares_for_investors} must be less or equal to shares supply: {}",
                shares.supply
            ));
        }

        let max_raisable_amount = FundsAmount::new(
            shares
                .supply
                .val()
                .checked_mul(share_price.val())
                .ok_or_else(|| anyhow!(""))?,
        );

        if raise_min_target.val() > max_raisable_amount.val() {
            return Err(anyhow!(
                "Min target: {} must be <= max possible funding (supply * price): {}",
                raise_min_target,
                max_raisable_amount
            ));
        }

        Ok(SetupDaoSpecs {
            name,
            descr_url,
            shares,
            investors_share,
            share_price,
            image_url,
            social_media_url,
            shares_for_investors,
            raise_min_target,
            raise_end_date,
        })
    }

    pub fn shares_for_investors(&self) -> ShareAmount {
        self.shares_for_investors
    }

    pub fn shares_for_creator(&self) -> ShareAmount {
        // we check in the initializer that supply >= investors_part, so this is safe
        ShareAmount::new(self.shares.supply.val() - self.shares_for_investors.val())
    }
}
