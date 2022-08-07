use algonaut::{
    core::Address,
    transaction::{contract_account::ContractAccount, SignedTransaction, Transaction},
};
use mbase::{
    api::version::VersionedContractAccount,
    models::{
        dao_app_id::DaoAppId,
        dao_id::DaoId,
        funds::{FundsAmount, FundsAssetId},
        nft::{Cid, Nft},
        setup_dao_specs::SetupDaoSpecs,
        share_amount::ShareAmount,
        shares_percentage::SharesPercentage,
        timestamp::Timestamp,
    },
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitSetupEscrowRes {
    pub shares_optin_escrow_algos_tx_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupInvestingEscrowToSign {
    pub escrow: VersionedContractAccount,
    pub escrow_shares_optin_tx: Transaction,
    pub escrow_funding_algos_tx: Transaction,
    pub escrow_funding_shares_asset_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupInvestEscrowSigned {
    pub escrow: ContractAccount,
    pub shares_optin_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupDaoToSign {
    pub fund_app_tx: Transaction,
    pub setup_app_tx: Transaction,
    pub transfer_shares_to_app_tx: Transaction,

    pub specs: SetupDaoSpecs,
    pub creator: Address,

    pub setup_date: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupDaoSigned {
    pub app_funding_tx: SignedTransaction,
    pub setup_app_tx: SignedTransaction,
    pub transfer_shares_to_app_tx: SignedTransaction,

    pub specs: SetupDaoSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub app_id: DaoAppId,
    pub funds_asset_id: FundsAssetId,
    pub image_url: Option<String>,

    pub setup_date: Timestamp,

    pub prospectus_url: Option<String>,
}

/// Note that dao doesn't know its id (DaoId), because it's generated after it's stored (it's the id of the storage tx),
/// TODO it probably makes sense to nane the id "StoredDaoId" to be more accurate.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dao {
    pub app_id: DaoAppId,
    pub owner: Address,
    pub shares_asset_id: u64,
    pub funds_asset_id: FundsAssetId,

    pub name: String,
    pub descr_url: Option<String>,
    pub token_name: String,
    pub token_supply: ShareAmount,
    pub investors_share: SharesPercentage,
    pub share_price: FundsAmount,
    pub image_nft: Option<Nft>,
    pub social_media_url: String, // this can be later in an extension (possibly with more links)
    // we manage this as timestamp instead of date,
    // to ensure correctness when storing the timestamp in TEAL / compare to current TEAL timestamp (which is in seconds)
    // DateTime can have millis and nanoseconds too,
    // which would e.g. break equality comparisons between these specs and the ones loaded from global state
    pub raise_end_date: Timestamp,
    pub raise_min_target: FundsAmount,

    pub raised: FundsAmount,

    pub setup_date: Timestamp,

    pub prospectus_url: Option<String>,
}

impl Dao {
    pub fn id(&self) -> DaoId {
        // we can repurpose the app id as dao id, because it's permanent and unique on the blockchain
        DaoId(self.app_id)
    }

    pub fn app_address(&self) -> Address {
        self.app_id.address()
    }
}

// Implemented manually to show the app address too (which is derived from the id)
impl Debug for Dao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dao")
            .field("app_id", &self.app_id)
            .field("app_address()", &self.app_address())
            .field("creator", &self.owner)
            .field("shares_asset_id", &self.shares_asset_id)
            .field("funds_asset_id", &self.funds_asset_id)
            .field("name", &self.name)
            .field("descr_url", &self.descr_url)
            .field("token_name", &self.token_name)
            .field("token_supply", &self.token_supply)
            .field("investors_share", &self.investors_share)
            .field("share_price", &self.share_price)
            .field("image_nft", &self.image_nft)
            .field("social_media_url", &self.social_media_url)
            .field("raise_end_date", &self.raise_end_date)
            .field("raise_min_target", &self.raise_min_target)
            .field("raised", &self.raised)
            .field("prospectus_url", &self.prospectus_url)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitSetupDaoResult {
    pub dao: Dao,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssetsToSign {
    pub create_shares_tx: Transaction,
    pub create_app_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateImageNftToSign {
    pub tx: Transaction,
    pub cid: Cid,
}
