use std::convert::TryInto;

use algonaut::{core::Address, indexer::v2::Indexer, model::indexer::v2::QueryAccountTransaction};
use chrono::{DateTime, NaiveDateTime, Utc};

use crate::{
    api::model::Withdrawal,
    withdrawal_note_prefix::{
        strip_prefix_from_note, withdrawal_tx_note_prefix_with_project_id_base64,
    },
};
use anyhow::{anyhow, Result};

pub async fn withdrawals(
    indexer: &Indexer,
    creator: &Address,
    project_id: u64,
) -> Result<Vec<Withdrawal>> {
    let mut query = QueryAccountTransaction::default();
    query.note_prefix = Some(withdrawal_tx_note_prefix_with_project_id_base64(project_id));

    let txs = indexer
        .account_transactions(creator, &query)
        .await?
        .transactions;

    let mut withdrawals = vec![];

    for tx in &txs {
        let payment = tx
            .payment_transaction
            .clone()
            .ok_or_else(|| anyhow!("Unexpected: transaction isn't a payment: {:?}", tx))?;

        // Unexpected because we just fetched by note prefix, so logically it should have a note
        let note = tx
            .note
            .clone()
            .ok_or_else(|| anyhow!("Unexpected: withdrawal tx has no note: {:?}", tx))?;

        // for now the only payload is the description
        let withdrawal_description = strip_prefix_from_note(&note.as_bytes(), project_id)?;

        // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
        // Unclear when it's None. For now we just reject it.
        let round_time = tx
            .round_time
            .clone()
            .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

        withdrawals.push(Withdrawal {
            project_id,
            amount: payment.amount,
            description: withdrawal_description,
            date: to_date(round_time)?,
        })
    }

    Ok(withdrawals)
}

fn to_date(timestamp: u64) -> Result<DateTime<Utc>> {
    // i64::MAX is in the year 2262, where if this program still exists and is regularly updated, the dependencies should require suitable types.
    // until then we don't expect this to fail (under normal circumstances).
    let timestamp_i64 = timestamp.try_into()?;
    Ok(DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp(timestamp_i64, 0),
        Utc,
    ))
}
