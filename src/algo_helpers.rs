use crate::network_util::wait_for_pending_transaction;
use algonaut::{
    algod::v2::Algod,
    core::{MicroAlgos, SuggestedTransactionParams},
    model::algod::v2::PendingTransaction,
    transaction::{SignedTransaction, Transaction},
};
use anyhow::{anyhow, Result};

/// Sums the estimated fees of all the passed transactions
pub fn calculate_total_fee(
    params: &SuggestedTransactionParams,
    txs: &[&mut Transaction],
) -> Result<MicroAlgos> {
    let mut total_fee = MicroAlgos(0);
    for tx in txs {
        total_fee = total_fee + tx.estimate_fee_with_params(&params)?;
    }
    log::debug!("Calculated total fee: {total_fee}");
    Ok(total_fee)
}

pub async fn send_and_retrieve_asset_id(algod: &Algod, tx: &SignedTransaction) -> Result<u64> {
    let p_tx = send_and_wait_for_pending_tx(algod, tx).await?;
    p_tx.asset_index
        .ok_or_else(|| anyhow!("Shares asset id in pending tx not set"))
}

pub async fn send_and_retrieve_app_id(algod: &Algod, tx: &SignedTransaction) -> Result<u64> {
    let p_tx = send_and_wait_for_pending_tx(algod, tx).await?;
    p_tx.application_index
        .ok_or_else(|| anyhow!("App id in pending tx not set"))
}

pub async fn send_and_wait_for_pending_tx(
    algod: &Algod,
    tx: &SignedTransaction,
) -> Result<PendingTransaction> {
    let res = algod.broadcast_signed_transaction(tx).await?;
    wait_for_pending_transaction(algod, &res.tx_id.parse()?)
        .await?
        .ok_or_else(|| anyhow!("No pending tx to retrieve asset_od"))
}
