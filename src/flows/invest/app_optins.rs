use crate::flows::{
    create_project::{model::Project, storage::load_project::TxId},
    shared::app::optin_to_app,
};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    transaction::{SignedTransaction, Transaction},
};
use anyhow::Result;

pub async fn invest_or_staking_app_optin_tx(
    algod: &Algod,
    project: &Project,
    investor: &Address,
) -> Result<Transaction> {
    let params = algod.suggested_transaction_params().await?;
    let central_optin_tx = optin_to_app(&params, project.central_app_id, *investor)?;
    Ok(central_optin_tx)
}

pub async fn submit_invest_or_staking_app_optin(
    algod: &Algod,
    signed: SignedTransaction,
) -> Result<TxId> {
    // crate::teal::debug_teal_rendered(&signed, "app_central_approval").unwrap();
    let res = algod.broadcast_signed_transaction(&signed).await?;
    log::debug!("Investor app optins tx id: {}", res.tx_id);
    Ok(res.tx_id.parse()?)
}
