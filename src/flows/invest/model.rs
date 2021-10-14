use crate::flows::create_project::model::Project;
use algonaut::transaction::{SignedTransaction, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestToSign {
    pub project: Project,
    pub central_app_setup_tx: Transaction,
    pub slots_setup_txs: Vec<Transaction>,
    pub payment_tx: Transaction,
    pub shares_asset_optin_tx: Transaction,
    pub pay_escrow_fee_tx: Transaction,
    pub shares_xfer_tx: SignedTransaction, // contract account logic sig
    pub votes_xfer_tx: SignedTransaction,  // contract account logic sig
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestSigned {
    pub project: Project,
    pub central_app_setup_tx: SignedTransaction,
    pub slots_setup_txs: Vec<SignedTransaction>,
    pub shares_asset_optin_tx: SignedTransaction,
    pub payment_tx: SignedTransaction,
    pub pay_escrow_fee_tx: SignedTransaction,
    pub shares_xfer_tx: SignedTransaction, // contract account logic sig
    pub votes_xfer_tx: SignedTransaction,  // contract account logic sig
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestResult {
    // TODO id of what tx? do we need this?
    // more generally for what do we need all these fields, if it's only for testing it should be somewhere else
    pub tx_id: String,
    pub project: Project,
    pub central_app_investor_setup_tx: SignedTransaction,
    pub slots_setup_txs: Vec<SignedTransaction>,
    pub payment_tx: SignedTransaction,
    pub shares_asset_optin_tx: SignedTransaction,
    pub pay_escrow_fee_tx: SignedTransaction,
    pub shares_xfer_tx: SignedTransaction, // contract account logic sig
    pub votes_xfer_tx: SignedTransaction,  // contract account logic sig
}
