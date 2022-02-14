use algonaut::{
    core::Address,
    indexer::v2::Indexer,
    model::indexer::v2::{QueryTransaction, Role},
};
use anyhow::{anyhow, Error, Result};
use chrono::{DateTime, Utc};

use crate::{
    date_util::timestamp_seconds_to_date, flows::create_project::storage::load_project::TxId,
    funds::FundsAmount,
};

/// Project payments, i.e. funds asset transfers
pub async fn received_payments(indexer: &Indexer, address: &Address) -> Result<Vec<Payment>> {
    log::debug!("Retrieving payment to: {:?}", address);

    let response = indexer
        .transactions(&QueryTransaction {
            address: Some(address.to_string()),
            address_role: Some(Role::Receiver),
            ..QueryTransaction::default()
        })
        .await?;

    let mut payments = vec![];
    for tx in &response.transactions {
        if let Some(payment_tx) = &tx.asset_transfer_transaction {
            let receiver_address = payment_tx.receiver.parse::<Address>().map_err(Error::msg)?;

            // Sanity check
            if &receiver_address != address {
                return Err(anyhow!(
                    "Invalid state: tx receiver isn't the receiver we sent in the query"
                ));
            }

            // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
            // Unclear when it's None. For now we just reject it.
            let round_time = tx
                .round_time
                .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

            payments.push(Payment {
                tx_id: tx.id.parse()?,
                amount: FundsAmount(payment_tx.amount),
                sender: tx.sender.parse().map_err(Error::msg)?,
                date: timestamp_seconds_to_date(round_time)?,
                note: tx.note.clone(),
            })
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
