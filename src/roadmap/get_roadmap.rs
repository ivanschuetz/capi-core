use super::{add_roadmap_item::RoadmapItem, note::base64_maybe_roadmap_note_to_roadmap_item};
use crate::{
    date_util::timestamp_seconds_to_date,
    flows::create_project::storage::load_project::{ProjectId, TxId},
};
use algonaut::{
    core::Address,
    crypto::HashDigest,
    indexer::v2::Indexer,
    model::indexer::v2::{QueryTransaction, Role},
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;

pub async fn get_roadmap(
    indexer: &Indexer,
    project_creator: &Address,
    project_id: &ProjectId,
) -> Result<Roadmap> {
    // We get all the txs sent by project's creator and filter manually by the project prefix
    // Algorand's indexer has performance problems with note-prefix and it doesn't work at all with AlgoExplorer or PureStake currently:
    // https://github.com/algorand/indexer/issues/358
    // https://github.com/algorand/indexer/issues/669

    let response = indexer
        .transactions(&QueryTransaction {
            address: Some(project_creator.to_string()),
            address_role: Some(Role::Sender),
            ..QueryTransaction::default()
        })
        .await?;

    let mut roadmap_items = vec![];

    for tx in response.transactions {
        // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
        // Unclear when it's None. For now we just reject it.
        let round_time = tx
            .round_time
            .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

        if tx.payment_transaction.is_some() {
            if let Some(note) = tx.note.clone() {
                if let Some(roadmap_item) =
                    base64_maybe_roadmap_note_to_roadmap_item(&note, project_id)?
                {
                    let saved_roadmap_item =
                        to_saved_roadmap_item(&roadmap_item, &tx.id.parse()?, round_time)?;
                    roadmap_items.push(saved_roadmap_item);
                }
            }
        }
    }

    Ok(Roadmap {
        items: roadmap_items,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Roadmap {
    pub items: Vec<SavedRoadmapItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedRoadmapItem {
    pub tx_id: TxId,
    pub project_id: ProjectId,
    pub title: String,
    pub date: DateTime<Utc>,
    pub saved_date: DateTime<Utc>,
    pub parent: Box<Option<HashDigest>>,
    pub hash: HashDigest,
}

fn to_saved_roadmap_item(
    item: &RoadmapItem,
    tx_id: &TxId,
    round_time: u64,
) -> Result<SavedRoadmapItem> {
    Ok(SavedRoadmapItem {
        tx_id: tx_id.clone(),
        project_id: item.project_id.clone(),
        title: item.title.clone(),
        date: item.date,
        saved_date: timestamp_seconds_to_date(round_time)?,
        parent: item.parent.clone(),
        hash: item.hash,
    })
}
