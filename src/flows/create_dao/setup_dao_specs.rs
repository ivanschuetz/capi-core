use super::model::CreateSharesSpecs;
use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use mbase::models::{
    funds::FundsAmount, image_hash::ImageHash, share_amount::ShareAmount,
    shares_percentage::SharesPercentage, timestamp::Timestamp,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;

/// Represents an image that was already compressed
/// and checked against its size limit (for now not enforced here - to simplify error typing)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressedImage(Vec<u8>);

impl CompressedImage {
    pub fn new(bytes: Vec<u8>) -> CompressedImage {
        CompressedImage(bytes)
    }

    pub fn hash(&self) -> ImageHash {
        let digest = sha2::Sha512_256::digest(&self.0);
        ImageHash(BASE64.encode(&digest))
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.0.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupDaoSpecs {
    pub name: String,
    pub description: String,
    pub shares: CreateSharesSpecs,
    pub investors_share: SharesPercentage,
    pub share_price: FundsAmount,
    pub image_hash: Option<ImageHash>,
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
    pub fn new(
        name: String,
        description: String,
        shares: CreateSharesSpecs,
        investors_share: SharesPercentage,
        share_price: FundsAmount,
        image_hash: Option<ImageHash>,
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

        Ok(SetupDaoSpecs {
            name,
            description,
            shares,
            investors_share,
            share_price,
            image_hash,
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
