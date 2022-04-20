use super::{
    setup_dao_specs::SetupDaoSpecs,
    share_amount::ShareAmount,
    storage::load_dao::{DaoAppId, DaoId},
};
use crate::{api::version::VersionedContractAccount, funds::FundsAssetId};
use algonaut::{
    core::Address,
    transaction::{contract_account::ContractAccount, SignedTransaction, Transaction},
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
    pub customer_escrow_funding_tx: Transaction,
    pub fund_app_tx: Transaction,
    pub setup_app_tx: Transaction,
    pub transfer_shares_to_app_tx: Transaction,

    pub customer_escrow_optin_to_funds_asset_tx: SignedTransaction, // lsig

    pub specs: SetupDaoSpecs,
    pub customer_escrow: VersionedContractAccount,
    pub creator: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupDaoSigned {
    pub app_funding_tx: SignedTransaction,
    pub fund_customer_escrow_tx: SignedTransaction,
    pub setup_app_tx: SignedTransaction,
    pub customer_escrow_optin_to_funds_asset_tx: SignedTransaction, // lsig
    pub transfer_shares_to_app_tx: SignedTransaction,

    pub specs: SetupDaoSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub app_id: DaoAppId,
    pub funds_asset_id: FundsAssetId,
    pub customer_escrow: VersionedContractAccount,
}

/// Note that dao doesn't know its id (DaoId), because it's generated after it's stored (it's the id of the storage tx),
/// TODO it probably makes sense to nane the id "StoredDaoId" to be more accurate.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dao {
    pub app_id: DaoAppId,
    pub specs: SetupDaoSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub funds_asset_id: FundsAssetId,
    pub customer_escrow: VersionedContractAccount,
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
            .field("specs", &self.specs)
            .field("creator", &self.creator)
            .field("shares_asset_id", &self.shares_asset_id)
            .field("funds_asset_id", &self.funds_asset_id)
            .field("customer_escrow", &self.customer_escrow)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitSetupDaoResult {
    pub dao: Dao,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateSharesSpecs {
    pub token_name: String,
    pub supply: ShareAmount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssetsToSign {
    pub create_shares_tx: Transaction,
    pub create_app_tx: Transaction,
}
