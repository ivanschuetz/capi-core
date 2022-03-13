use crate::flows::{
    create_dao::{model::Dao, storage::load_dao::TxId},
    shared::app::optin_to_app,
};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    transaction::{SignedTransaction, Transaction},
};
use anyhow::Result;

pub async fn invest_or_locking_app_optin_tx(
    algod: &Algod,
    dao: &Dao,
    investor: &Address,
) -> Result<Transaction> {
    let params = algod.suggested_transaction_params().await?;
    let central_optin_tx = optin_to_app(&params, dao.central_app_id, *investor).await?;
    Ok(central_optin_tx)
}

pub async fn submit_invest_or_locking_app_optin(
    algod: &Algod,
    signed: SignedTransaction,
) -> Result<TxId> {
    // crate::teal::debug_teal_rendered(&signed, "app_central_approval").unwrap();
    let res = algod.broadcast_signed_transaction(&signed).await?;
    log::debug!("Investor app optins tx id: {}", res.tx_id);
    Ok(res.tx_id.parse()?)
}
