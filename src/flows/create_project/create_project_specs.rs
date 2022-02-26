use super::{model::CreateSharesSpecs, share_amount::ShareAmount};
use crate::funds::FundsAmount;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateProjectSpecs {
    pub name: String,
    pub description: String,
    pub shares: CreateSharesSpecs,
    investors_part: ShareAmount, // one private field, to prevent raw initialization
    pub share_price: FundsAmount,
    pub logo_url: String, // TODO limit size (this is stored in note) - maybe use newtype
    pub social_media_url: String, // this can be later in an extension (possibly with more links)
}

impl CreateProjectSpecs {
    pub fn new(
        name: String,
        description: String,
        shares: CreateSharesSpecs,
        investors_part: ShareAmount,
        share_price: FundsAmount,
        logo_url: String,
        social_media_url: String,
    ) -> Result<CreateProjectSpecs> {
        if investors_part > shares.supply {
            return Err(anyhow!(
                "Investors shares: {investors_part} must be less than shares supply: {}",
                shares.supply
            ));
        }
        Ok(CreateProjectSpecs {
            name,
            description,
            shares,
            investors_part,
            share_price,
            logo_url,
            social_media_url,
        })
    }

    pub fn creator_part(&self) -> ShareAmount {
        // we check in the initializer that count >= investors.count, so this is safe
        ShareAmount::new(self.shares.supply.val() - self.investors_part.val())
    }

    pub fn investors_part(&self) -> ShareAmount {
        self.investors_part
    }
}
