use algonaut::{
    core::{MicroAlgos, SuggestedTransactionParams},
    transaction::Transaction,
};
use anyhow::Result;

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
