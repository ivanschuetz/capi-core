use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    indexer::v2::Indexer,
    model::indexer::v2::QueryAccountTransaction,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    date_util::timestamp_seconds_to_date,
    flows::create_project::{
        create_project::Escrows,
        storage::load_project::{load_project, ProjectHash},
    },
    tx_note::{strip_withdraw_prefix_from_note, withdraw_note_prefix_base64},
};
use anyhow::{anyhow, Error, Result};

pub async fn withdrawals(
    algod: &Algod,
    indexer: &Indexer,
    creator: &Address,
    project_hash: &ProjectHash,
    escrows: &Escrows,
) -> Result<Vec<Withdrawal>> {
    log::debug!(
        "Querying withdrawals by: {:?} for project: {:?}",
        creator,
        project_hash.url_str()
    );

    let project = load_project(algod, indexer, project_hash, escrows).await?;

    let query = QueryAccountTransaction {
        note_prefix: Some(withdraw_note_prefix_base64()),
        ..Default::default()
    };

    let txs = indexer
        .account_transactions(creator, &query)
        .await?
        .transactions;

    let mut withdrawals = vec![];

    for tx in &txs {
        // withdrawals are payments - ignore other txs
        if let Some(payment) = tx.payment_transaction.clone() {
            let sender_address = tx.sender.parse::<Address>().map_err(Error::msg)?;
            let receiver_address = payment.receiver.parse::<Address>().map_err(Error::msg)?;

            // account_transactions returns all the txs "related" to the account, i.e. can be sender or receiver
            // we're interested only in central escrow -> creator
            if sender_address == *project.central_escrow.address() && receiver_address == *creator {
                // Unexpected because we just fetched by (a non empty) note prefix, so logically it should have a note
                let note = tx
                    .note
                    .clone()
                    .ok_or_else(|| anyhow!("Unexpected: withdrawal tx has no note: {:?}", tx))?;

                // for now the only payload is the description
                let withdrawal_description = strip_withdraw_prefix_from_note(note.as_bytes())?;

                // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
                // Unclear when it's None. For now we just reject it.
                let round_time = tx
                    .round_time
                    .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

                withdrawals.push(Withdrawal {
                    project_uuid: project.uuid,
                    amount: payment.amount,
                    description: withdrawal_description,
                    date: timestamp_seconds_to_date(round_time)?,
                    tx_id: tx.id.clone(),
                })
            }
        }
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
