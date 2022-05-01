use super::{
    model::CreateSharesSpecs, share_amount::ShareAmount, shares_percentage::SharesPercentage,
};
use crate::{api::image_api::ImageApi, funds::FundsAmount};
use anyhow::{anyhow, Result};
use data_encoding::BASE64;
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

/// Assumes string to be base64 encoded hash bytes
/// we might change this in the future to store and handle directly the hash bytes (similar to Algonaut's HashDigest struct)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageHash(pub String);

impl ImageHash {
    pub fn bytes(&self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<ImageHash> {
        Ok(ImageHash(String::from_utf8(bytes)?))
    }

    pub fn as_str(&self) -> String {
        self.0.clone()
    }

    pub fn as_api_id(&self) -> String {
        self.0.clone()
    }

    pub fn as_api_url(&self, image_api: &dyn ImageApi) -> String {
        image_api.image_url(&self.as_api_id())
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
