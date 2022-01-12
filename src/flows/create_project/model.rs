use algonaut::{
    core::{Address, MicroAlgos},
    crypto::HashDigest,
    transaction::{contract_account::ContractAccount, SignedTransaction, Transaction},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::hashable::Hashable;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateProjectSpecs {
    pub name: String,
    pub shares: CreateSharesSpecs,
    pub asset_price: MicroAlgos,
    pub investors_share: u64, // percentage as entered by the user, e.g. 30%. No fractionals.
                              // TODO maybe use Decimal, ensure valid range (1..100)
}

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
pub struct CreateProjectToSign {
    // to be signed by creator
    pub escrow_funding_txs: Vec<Transaction>,
    pub create_app_tx: Transaction,
    pub xfer_shares_to_invest_escrow: Transaction,

    // escrow optins (lsig)
    // (note that "to sign" in struct's name means that there are _some_ txs to sign. this is just passtrough data)
    pub optin_txs: Vec<SignedTransaction>,

    pub specs: CreateProjectSpecs,
    pub staking_escrow: ContractAccount,
    pub invest_escrow: ContractAccount,
    pub central_escrow: ContractAccount,
    pub customer_escrow: ContractAccount,
    pub creator: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateProjectSigned {
    //////////////////////////////////////////////
    // transactions to be submitted
    //////////////////////////////////////////////
    // escrow funding txs (sent by creator)
    pub escrow_funding_txs: Vec<SignedTransaction>,

    // fund the investing escrow with assets: dedicated fields, to be executed after the asset opt-in
    pub xfer_shares_to_invest_escrow: SignedTransaction,

    // create the central app: dedicated field to get the app id (when in a group, the pending tx doesn't deliver it - TODO confirm)
    // see more notes in old repo
    pub create_app_tx: SignedTransaction,

    // escrows opt-in (lsig - signed when created)
    // to be submitted before possible asset transfers
    // on project creation assets are transferred only to investing escrow,
    // we opt-in all the escrows that may touch the assets later here too, just to leave the system "initialized"
    pub optin_txs: Vec<SignedTransaction>,

    //////////////////////////////////////////////
    // passthrough
    //////////////////////////////////////////////
    pub specs: CreateProjectSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub invest_escrow: ContractAccount,
    pub staking_escrow: ContractAccount,
    pub central_escrow: ContractAccount,
    pub customer_escrow: ContractAccount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub specs: CreateProjectSpecs,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub central_app_id: u64,
    pub invest_escrow: ContractAccount,
    pub staking_escrow: ContractAccount,
    pub central_escrow: ContractAccount,
    pub customer_escrow: ContractAccount,
}

impl Project {
    pub fn hash(&self) -> Result<HashDigest> {
        Ok(*self.compute_hash()?.hash())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitCreateProjectResult {
    pub project: Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateSharesSpecs {
    pub token_name: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSharesToSign {
    pub create_shares_tx: Transaction,
}
