use super::model::CreateSharesSpecs;
use crate::funds::FundsAmount;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateProjectSpecs {
    pub name: String,
    pub description: String,
    pub shares: CreateSharesSpecs,
    investors_part: u64, // one private field, to prevent raw initialization
    pub share_price: FundsAmount,
    pub logo_url: String, // TODO limit size (this is stored in note) - maybe use newtype
    pub social_media_url: String, // this can be later in an extension (possibly with more links)
}

impl CreateProjectSpecs {
    pub fn new(
        name: String,
        description: String,
        shares: CreateSharesSpecs,
        investors_part: u64,
        share_price: FundsAmount,
        logo_url: String,
        social_media_url: String,
    ) -> Result<CreateProjectSpecs> {
        if investors_part > shares.count {
            return Err(anyhow!(
                "Investors shares: {investors_part} must be less than shares supply: {}",
                shares.count
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

    pub fn creator_part(&self) -> u64 {
        // we check in the initializer that count >= investors.count, so this is safe
        self.shares.count - self.investors_part
    }

    pub fn investors_part(&self) -> u64 {
        self.investors_part
    }
}
