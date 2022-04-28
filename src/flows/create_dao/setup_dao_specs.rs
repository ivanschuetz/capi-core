use super::{
    model::CreateSharesSpecs, share_amount::ShareAmount, shares_percentage::SharesPercentage,
};
use crate::funds::FundsAmount;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupDaoSpecs {
    pub name: String,
    pub description: String,
    pub shares: CreateSharesSpecs,
    pub investors_share: SharesPercentage,
    pub share_price: FundsAmount,
    pub logo_url: String, // TODO limit size (this is stored in note) - maybe use newtype
    pub social_media_url: String, // this can be later in an extension (possibly with more links)
    // shares to be sold to investors (the rest stay in the creator's account)
    // note this is entirely different from investors_share, which is the % of the project's income channeled to investors
    shares_for_investors: ShareAmount,
}

impl SetupDaoSpecs {
    pub fn new(
        name: String,
        description: String,
        shares: CreateSharesSpecs,
        investors_share: SharesPercentage,
        share_price: FundsAmount,
        logo_url: String,
        social_media_url: String,
        shares_for_investors: ShareAmount,
    ) -> Result<SetupDaoSpecs> {
        if shares_for_investors > shares.supply {
            return Err(anyhow!(
                "Shares for investors: {shares_for_investors} must be less or equal to shares supply: {}",
                shares.supply
            ));
        }
        Ok(SetupDaoSpecs {
            name,
            description,
            shares,
            investors_share,
            share_price,
            logo_url,
            social_media_url,
            shares_for_investors,
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
