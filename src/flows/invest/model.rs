use crate::flows::create_dao::{model::Dao, storage::load_dao::TxId};
use algonaut::transaction::{SignedTransaction, Transaction};
use mbase::models::funds::FundsAmount;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestToSign {
    pub dao: Dao,
    pub central_app_setup_tx: Transaction,
    pub payment_tx: Transaction,
    pub shares_asset_optin_tx: Transaction,
    // the total price paid for the shares is calculated when generating the txs,
    // (based on the share count parameter and the share's price, which is in the dao)
    pub total_price: FundsAmount
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestSigned {
    pub dao: Dao,
    pub central_app_setup_tx: SignedTransaction,
    pub shares_asset_optin_tx: SignedTransaction,
    pub payment_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestResult {
    // TODO id of what tx? do we need this?
    // more generally for what do we need all these fields, if it's only for testing it should be somewhere else
    pub tx_id: TxId,
    pub dao: Dao,
    pub central_app_investor_setup_tx: SignedTransaction,
    pub payment_tx: SignedTransaction,
    pub shares_asset_optin_tx: SignedTransaction,
}
