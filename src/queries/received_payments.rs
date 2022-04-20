use algonaut::{core::Address, indexer::v2::Indexer, model::indexer::v2::QueryTransaction};
use anyhow::{anyhow, Error, Result};
use chrono::{DateTime, Utc};

use crate::{
    date_util::timestamp_seconds_to_date,
    flows::create_dao::storage::load_dao::TxId,
    funds::{FundsAmount, FundsAssetId},
};

/// Payments (funds xfer) to the Dao
pub async fn received_payments(
    indexer: &Indexer,
    address: &Address,
    funds_asset: FundsAssetId,
) -> Result<Vec<Payment>> {
    log::debug!("Retrieving payment to: {:?}", address);

    let response = indexer
        .transactions(&QueryTransaction {
            address: Some(address.to_string()),
            // indexer disabled this, for performance apparently https://github.com/algorand/indexer/commit/1216e7957d5fba7c6a858e244a2aaf7e99412e5d
            // so we filter locally
            // address_role: Some(Role::Receiver),
            ..QueryTransaction::default()
        })
        .await?;

    let mut payments = vec![];
    for tx in &response.transactions {
        let sender_address = tx.sender.parse::<Address>().map_err(Error::msg)?;

        if let Some(xfer_tx) = &tx.asset_transfer_transaction {
            let receiver_address = xfer_tx.receiver.parse::<Address>().map_err(Error::msg)?;

            if &receiver_address == address && xfer_tx.asset_id == funds_asset.0 {
                // Skip asset opt-ins
                if sender_address == receiver_address && xfer_tx.amount == 0 {
                    continue;
                }

                // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
                // Unclear when it's None. For now we just reject it.
                let round_time = tx
                    .round_time
                    .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

                let id = tx
                    .id
                    .clone()
                    .ok_or_else(|| anyhow!("Unexpected: tx has no id: {:?}", tx))?;

                payments.push(Payment {
                    tx_id: id.parse()?,
                    amount: FundsAmount::new(xfer_tx.amount),
                    sender: tx.sender.parse().map_err(Error::msg)?,
                    date: timestamp_seconds_to_date(round_time)?,
                    note: tx.note.clone(),
                })
            }
        } else {
            // Just a "why not" log - e.g. if we're debugging the customer escrow payments,
            // it can be worth inspecting non payment txs as their purpose would be unclear.
            log::trace!("Payment receiver received a non-payment tx: {:?}", tx);
        }
    }
    Ok(payments)
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub tx_id: TxId,
    pub amount: FundsAmount,
    pub sender: Address,
    pub date: DateTime<Utc>,
    pub note: Option<String>,
}
