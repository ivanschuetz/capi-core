use super::{model::CreateSharesSpecs, shares_percentage::SharesPercentage};
use crate::funds::FundsAmount;
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
}
