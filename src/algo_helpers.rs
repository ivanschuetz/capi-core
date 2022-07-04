use crate::{
    flows::create_dao::storage::load_dao::TxId, network_util::wait_for_pending_transaction,
};
use algonaut::{
    algod::v2::Algod,
    core::{MicroAlgos, SuggestedTransactionParams},
    model::algod::v2::{PendingTransaction, TransactionResponse},
    transaction::{SignedTransaction, Transaction},
};
use anyhow::{anyhow, Result};

/// Sums the estimated fees of all the passed transactions
pub fn calculate_total_fee(
    params: &SuggestedTransactionParams,
    txs: &[&Transaction],
) -> Result<MicroAlgos> {
    let mut total_fee = MicroAlgos(0);
    for tx in txs {
        total_fee = total_fee + tx.estimate_basic_sig_fee_with_params(params)?;
    }
    log::debug!("Calculated total fee: {total_fee}");
    Ok(total_fee)
}

pub async fn send_tx_and_wait(algod: &Algod, tx: &SignedTransaction) -> Result<PendingTransaction> {
    let res = algod.broadcast_signed_transaction(tx).await?;
    wait_for_p_tx(algod, res).await
}

pub async fn send_txs_and_wait(
    algod: &Algod,
    txs: &[SignedTransaction],
) -> Result<PendingTransaction> {
    let res = algod.broadcast_signed_transactions(txs).await?;
    wait_for_p_tx(algod, res).await
}

async fn wait_for_p_tx(algod: &Algod, response: TransactionResponse) -> Result<PendingTransaction> {
    wait_for_p_tx_with_id(algod, &response.tx_id.parse()?).await
}

pub async fn wait_for_p_tx_with_id(algod: &Algod, tx_id: &TxId) -> Result<PendingTransaction> {
    let p_tx = wait_for_pending_transaction(algod, tx_id).await?;
    p_tx.ok_or_else(|| anyhow!("Pending tx couldn't be retrieved, tx id: {:?}", tx_id))
}
