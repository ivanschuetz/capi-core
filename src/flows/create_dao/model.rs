use super::{create_dao_specs::CreateDaoSpecs, share_amount::ShareAmount};
use crate::{funds::FundsAssetId, hashable::Hashable};
use algonaut::{
    core::Address,
    crypto::HashDigest,
    transaction::{contract_account::ContractAccount, SignedTransaction, Transaction},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitSetupEscrowRes {
    pub shares_optin_escrow_algos_tx_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupInvestingEscrowToSign {
    pub escrow: ContractAccount,
    pub escrow_shares_optin_tx: Transaction,
    // min amount to hold asset (shares) + asset optin tx fee
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
    // to be signed by creator
    pub escrow_funding_txs: Vec<Transaction>,
    pub setup_app_tx: Transaction,
    pub xfer_shares_to_invest_escrow: Transaction,

    // escrow optins (lsig)
    // (note that "to sign" in struct's name means that there are _some_ txs to sign. this is just passtrough data)
    pub optin_txs: Vec<SignedTransaction>,

    pub specs: CreateDaoSpecs,
    pub locking_escrow: ContractAccount,
    pub invest_escrow: ContractAccount,
    pub central_escrow: ContractAccount,
    pub customer_escrow: ContractAccount,
    pub creator: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateDaoSigned {
    //////////////////////////////////////////////
    // transactions to be submitted
    //////////////////////////////////////////////
    // escrow funding txs (sent by creator)
    pub escrow_funding_txs: Vec<SignedTransaction>,

    // fund the investing escrow with assets: dedicated fields, to be executed after the asset opt-in
    pub xfer_shares_to_invest_escrow: SignedTransaction,

    pub setup_app_tx: SignedTransaction,

    // escrows opt-in (lsig - signed when created)
    // to be submitted before possible asset transfers
    // on dao creation assets are transferred only to investing escrow,
    // we opt-in all the escrows that may touch the assets later here too, just to leave the system "initialized"
    pub optin_txs: Vec<SignedTransaction>,

    //////////////////////////////////////////////
    // passthrough
    //////////////////////////////////////////////
    pub specs: CreateDaoSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub central_app_id: u64,
    pub funds_asset_id: FundsAssetId,
    pub invest_escrow: ContractAccount,
    pub locking_escrow: ContractAccount,
    pub central_escrow: ContractAccount,
    pub customer_escrow: ContractAccount,
}

/// Note that dao doesn't know its id (DaoId), because it's generated after it's stored (it's the id of the storage tx),
/// TODO it probably makes sense to nane the id "StoredDaoId" to be more accurate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dao {
    pub specs: CreateDaoSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub funds_asset_id: FundsAssetId,
    pub central_app_id: u64,
    pub invest_escrow: ContractAccount,
    pub locking_escrow: ContractAccount,
    pub central_escrow: ContractAccount,
    pub customer_escrow: ContractAccount,
}

impl Dao {
    pub fn hash(&self) -> Result<HashDigest> {
        Ok(*self.compute_hash()?.hash())
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
