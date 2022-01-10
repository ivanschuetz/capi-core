use algonaut::{
    core::{Address, MicroAlgos},
    indexer::v2::Indexer,
    model::indexer::v2::QueryAccountTransaction,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    date_util::timestamp_seconds_to_date,
    tx_note::{project_uuid_note_prefix_base64, strip_prefix_from_note},
};
use anyhow::{anyhow, Result};

// TODO user project hash instead of uuid?
pub async fn withdrawals(
    indexer: &Indexer,
    creator: &Address,
    project_uuid: &Uuid,
) -> Result<Vec<Withdrawal>> {
    log::debug!(
        "Querying withdrawals by: {:?} for project: {:?}",
        creator,
        project_uuid
    );

    let query = QueryAccountTransaction {
        note_prefix: Some(project_uuid_note_prefix_base64(project_uuid)),
        ..Default::default()
    };

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

        // Unexpected because we just fetched by (a non empty) note prefix, so logically it should have a note
        let note = tx
            .note
            .clone()
            .ok_or_else(|| anyhow!("Unexpected: withdrawal tx has no note: {:?}", tx))?;

        // for now the only payload is the description
        let withdrawal_description = strip_prefix_from_note(note.as_bytes(), project_uuid)?;

        // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
        // Unclear when it's None. For now we just reject it.
        let round_time = tx
            .round_time
            .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

        withdrawals.push(Withdrawal {
            project_uuid: project_uuid.to_owned(),
            amount: payment.amount,
            description: withdrawal_description,
            date: timestamp_seconds_to_date(round_time)?,
            tx_id: tx.id.clone(),
        })
    }

    Ok(withdrawals)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Withdrawal {
    pub project_uuid: Uuid,
    pub amount: MicroAlgos,
    pub description: String,
    pub date: DateTime<Utc>,
    pub tx_id: String,
}
