use crate::{
    capi_deps::CapiAssetDaoDeps,
    flows::{
        create_dao::storage::load_dao::TxId,
        drain::drain::calculate_dao_and_capi_escrow_xfer_amounts,
    },
};
use algonaut::{core::Address, indexer::v2::Indexer, model::indexer::v2::QueryTransaction};
use anyhow::{anyhow, Error, Result};
use chrono::{DateTime, Utc};
use mbase::{
    checked::CheckedSub,
    date_util::timestamp_seconds_to_date,
    models::funds::{FundsAmount, FundsAssetId},
};

/// All the payments (funds xfer) made to the Dao
/// This combines payments to 2 escrows: the customer and the app's escrow
/// The payments to the app escrow coming from the customer escrow are removed, in order to not duplicate the customer escrow's payments
/// (note that we need to query both escrows, as there can be funds in the customer escrow that haven't been drained yet)
/// this way dates also make sense: customer payments have the date of the actual payment (xfer to customer escrow), not the draining date.
/// (and investors buying shares, expectedly also have the date of when the share was bought)
pub async fn all_received_payments(
    indexer: &Indexer,
    dao_address: &Address,
    customer_escrow_address: &Address,
    funds_asset: FundsAssetId,
    before_time: &Option<DateTime<Utc>>,
    after_time: &Option<DateTime<Utc>>,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Vec<Payment>> {
    // payments to the customer escrow
    let mut customer_escrow_payments = received_payments(
        indexer,
        customer_escrow_address,
        funds_asset,
        before_time,
        after_time,
        capi_deps,
        true,
    )
    .await?;
    // payments to the app escrow (either from investors buying shares, draining from customer escrow, or unexpected/not supported by the app payments)
    let app_escrow_payments = received_payments(
        indexer,
        dao_address,
        funds_asset,
        before_time,
        after_time,
        capi_deps,
        false,
    )
    .await?;
    // filter out draining (payments from customer escrow to app escrow), which would duplicate payments to the customer escrow
    let filtered_app_escrow_payments: Vec<Payment> = app_escrow_payments
        .into_iter()
        .filter(|p| &p.sender != customer_escrow_address)
        .collect();
    customer_escrow_payments.extend(filtered_app_escrow_payments);
    Ok(customer_escrow_payments)
}

/// Payments (funds xfer) to the Dao
async fn received_payments(
    indexer: &Indexer,
    address: &Address,
    funds_asset: FundsAssetId,
    before_time: &Option<DateTime<Utc>>,
    after_time: &Option<DateTime<Utc>>,
    capi_deps: &CapiAssetDaoDeps,
    to_customer_escrow: bool,
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

                // for customer escrow payments, we track the fee, to show on the UI (as otherwise possible confusion with actual project funds)
                // NOTE that in reality this fee isn't sent here (payment to customer escrow), but when *draining*
                // but since we remove the draining payments (xfers customer escrow -> app) (as we want the totality of payments and there may be undrained payments),
                // we've to apply the fee in advance here
                // practically it's correct, because no one can do anything with the escrow funds other than draining them,
                // so (for the purpose of tracking the project's income) it's like they had already paid the fee.
                //
                // WARNING / TODO: this assumes a never-changing capi fee,
                // if the fee was changed, we could be substracting the current fee from payments that were made with a different fee,
                // making our activity protocol inaccurate. For now (MVP) like this.
                //
                // TODO this is a bit hacky / too specific here, as it's supposed to be general payments for any addresses
                // for now like this. we probably should map the payments to "payments with fee" in a separate iteration
                let fee = if to_customer_escrow {
                    calculate_dao_and_capi_escrow_xfer_amounts(amount, capi_deps.escrow_percentage)?
                        .capi
                } else {
                    FundsAmount::new(0)
                };

                payments.push(Payment {
                    tx_id: id.parse()?,
                    amount,
                    sender: tx.sender.parse().map_err(Error::msg)?,
                    date: timestamp_seconds_to_date(round_time)?,
                    note: tx.note.clone(),
                    fee,
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
    pub fee: FundsAmount,
}

impl Payment {
    pub fn received_amount(&self) -> Result<FundsAmount> {
        self.amount.sub(&self.fee)
    }
}
