use super::{
    create_dao_specs::CreateDaoSpecs,
    share_amount::ShareAmount,
    storage::load_dao::{DaoAppId, DaoId},
};
use crate::{api::version::VersionedContractAccount, funds::FundsAssetId};
use algonaut::{
    core::Address,
    transaction::{contract_account::ContractAccount, SignedTransaction, Transaction},
};
use serde::{Deserialize, Serialize};

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
pub struct CreateDaoToSign {
    pub escrow_funding_txs: Vec<Transaction>,
    pub fund_app_tx: Transaction,
    pub setup_app_tx: Transaction,
    pub xfer_shares_to_invest_escrow: Transaction,

    // (note that "to sign" in struct's name means that there are _some_ txs to sign. this is just passtrough data)
    pub optin_txs: Vec<SignedTransaction>,

    pub specs: CreateDaoSpecs,
    pub locking_escrow: VersionedContractAccount,
    pub invest_escrow: VersionedContractAccount,
    pub customer_escrow: VersionedContractAccount,
    pub creator: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateDaoSigned {
    pub app_funding_tx: SignedTransaction,
    pub escrow_funding_txs: Vec<SignedTransaction>,
    pub xfer_shares_to_invest_escrow: SignedTransaction,
    pub setup_app_tx: SignedTransaction,
    pub optin_txs: Vec<SignedTransaction>,

    pub specs: CreateDaoSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub app_id: DaoAppId,
    pub funds_asset_id: FundsAssetId,
    pub invest_escrow: VersionedContractAccount,
    pub locking_escrow: VersionedContractAccount,
    pub customer_escrow: VersionedContractAccount,
}

/// Note that dao doesn't know its id (DaoId), because it's generated after it's stored (it's the id of the storage tx),
/// TODO it probably makes sense to nane the id "StoredDaoId" to be more accurate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dao {
    pub app_id: DaoAppId,
    pub specs: CreateDaoSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub funds_asset_id: FundsAssetId,
    pub invest_escrow: VersionedContractAccount,
    pub locking_escrow: VersionedContractAccount,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitCreateDaoResult {
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
