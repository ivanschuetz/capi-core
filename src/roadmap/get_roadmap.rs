use algonaut::{
    core::Address,
    crypto::HashDigest,
    indexer::v2::Indexer,
    model::indexer::v2::{QueryTransaction, Role},
};
use anyhow::{anyhow, Error, Result};
use chrono::{DateTime, Utc};
use data_encoding::BASE64;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    date_util::timestamp_seconds_to_date,
    tx_note::{project_uuid_note_prefix_base64, strip_prefix_from_note},
};

use super::add_roadmap_item::RoadmapItem;

pub async fn get_roadmap(
    indexer: &Indexer,
    project_creator: &Address,
    project_uuid: &Uuid,
) -> Result<Roadmap> {
    let note_prefix = project_uuid_note_prefix_base64(project_uuid);
    log::debug!(
        "Feching roadmap with prefix: {:?}, sender: {:?}, uuid (encoded in prefix): {:?}",
        note_prefix,
        project_creator,
        project_uuid
    );

    let response = indexer
        .transactions(&QueryTransaction {
            address: Some(project_creator.to_string()),
            address_role: Some(Role::Sender),
            note_prefix: Some(note_prefix),
            ..QueryTransaction::default()
        })
        .await?;

    let mut items = vec![];
    for tx in &response.transactions {
        if let Some(_) = &tx.payment_transaction {
            let sender_address = tx.sender.parse::<Address>().map_err(Error::msg)?;

            // Sanity check
            if &sender_address != project_creator {
                return Err(anyhow!(
                    "Invalid state: tx sender isn't the sender we sent in the query"
                ));
            }

            // Unexpected because we just fetched by (a non empty) note prefix, so logically it should have a note
            let note = tx
                .note
                .clone()
                .ok_or_else(|| anyhow!("Unexpected: roadmap tx has no note: {:?}", tx))?;

            let note_payload_str = strip_prefix_from_note(note.as_bytes(), project_uuid)?;
            let msg_pack_to_deserialize = BASE64.decode(note_payload_str.as_bytes())?;
            let roadmap_item = rmp_serde::from_slice::<RoadmapItem>(&msg_pack_to_deserialize)?;

            // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
            // Unclear when it's None. For now we just reject it.
            let round_time = tx
                .round_time
                .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

            let saved_roadmap_item = to_saved_roadmap_item(&roadmap_item, round_time)?;

            items.push(saved_roadmap_item)
        } else {
            // It can be worth inspecting these, as their purpose would be unclear.
            // if creator add roadmap items with our UI, the txs will always be payments (unless there's a bug).
            log::trace!("Roadmap txs query returned a non-payment tx: {:?}", tx);
        }
    }

    Ok(Roadmap { items })
}

#[derive(Debug, Clone, Serialize)]
pub struct Roadmap {
    pub items: Vec<SavedRoadmapItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedRoadmapItem {
    pub project_uuid: Uuid,
    pub title: String,
    pub date: DateTime<Utc>,
    pub parent: Box<Option<HashDigest>>,
    pub hash: HashDigest,
}

fn to_saved_roadmap_item(item: &RoadmapItem, round_time: u64) -> Result<SavedRoadmapItem> {
    Ok(SavedRoadmapItem {
        project_uuid: item.project_uuid.clone(),
        title: item.title.clone(),
        date: timestamp_seconds_to_date(round_time)?,
        parent: item.parent.clone(),
        hash: item.hash.clone(),
    })
}
