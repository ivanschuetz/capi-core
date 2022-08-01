use crate::flows::drain::drain::calculate_dao_and_capi_escrow_xfer_amounts;
use algonaut::{core::Address, indexer::v2::Indexer, model::indexer::v2::QueryTransaction};
use anyhow::{anyhow, Error, Result};
use chrono::{DateTime, Utc};
use data_encoding::BASE64;
use mbase::{
    checked::CheckedSub,
    date_util::timestamp_seconds_to_date,
    models::{funds::{FundsAmount, FundsAssetId}, capi_deps::CapiAssetDaoDeps, tx_id::TxId},
};

/// Payments (funds xfer) to the Dao escrow
pub async fn received_payments(
    indexer: &Indexer,
    address: &Address,
    funds_asset: FundsAssetId,
    before_time: &Option<DateTime<Utc>>,
    after_time: &Option<DateTime<Utc>>,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Vec<Payment>> {
    log::debug!("Retrieving payment to: {:?}", address);

    // let before_time_formatted = before_time.map(|t| t.to_rfc3339());
    // let after_time_formatted = after_time.map(|t| t.to_rfc3339());

    let response = indexer
        .transactions(&QueryTransaction {
            address: Some(address.to_string()),
            // added to disabled_parameters..
            // before_time: before_time_formatted,
            // added to disabled_parameters..
            // after_time: after_time_formatted,
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

            // funds asset, to the app
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

                // needs to be checked manually, because the query param was disabled
                if let Some(after_time) = after_time {
                    if round_time < after_time.timestamp() as u64 {
                        continue;
                    }
                }
                // needs to be checked manually, because the query param was disabled
                if let Some(before_time) = before_time {
                    if round_time > before_time.timestamp() as u64 {
                        continue;
                    }
                }

                let amount = FundsAmount::new(xfer_tx.amount);

                // investment funds don't pay a fee - they're added immediately to withdrawable amount
                // all other funds transfers to the app escrow have to go through draining and pay a fee
                // TODO (low prio)?: Note that anyone can send funds transfers to the app escrow with an "invest" note
                // making them be handled here as fee-less
                // this might be malicious to skew the statistics / funds history, for some reason
                // we might have to check for investments in a more robust way
                // e.g. checking the other txs in the group (https://github.com/algorand/indexer/issues/135)
                let fee = if tx.note == Some(BASE64.encode("Invest".as_bytes()).to_owned()) {
                    FundsAmount::new(0)
                } else {
                    calculate_dao_and_capi_escrow_xfer_amounts(amount, capi_deps.escrow_percentage)?
                        .capi
                };

                let note = if let Some(note) = &tx.note {
                    Some(String::from_utf8(BASE64.decode(note.as_bytes())?)?)
                } else {
                    None
                };

                payments.push(Payment {
                    tx_id: id.parse()?,
                    amount,
                    sender: tx.sender.parse().map_err(Error::msg)?,
                    date: timestamp_seconds_to_date(round_time)?,
                    note,
                    fee,
                })
            }
        } else {
            // it can be worth inspecting non transfer txs as their purpose would be unclear.
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
    // capi fee
    pub fee: FundsAmount,
}

impl Payment {
    pub fn received_amount(&self) -> Result<FundsAmount> {
        self.amount.sub(&self.fee)
    }
}
