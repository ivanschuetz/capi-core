use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    crypto::HashDigest,
    transaction::{Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use data_encoding::BASE64;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use uuid::Uuid;

use crate::tx_note::project_uuid_note_prefix;

pub async fn add_roadmap_item(
    algod: &Algod,
    project_creator: &Address,
    item_inputs: &RoadmapItemInputs,
) -> Result<AddRoadmapItemToSign> {
    let params = algod.suggested_transaction_params().await?;

    let roadmap_item = item_inputs.to_roadmap_item()?;
    let note = roadmap_item_as_tx_note(&roadmap_item)?;

    // 0 payment to themselves - we use a minimal tx only to store data.
    let tx = TxnBuilder::with(
        params.to_owned(),
        Pay::new(*project_creator, *project_creator, MicroAlgos(0)).build(),
    )
    .note(note)
    .build();

    Ok(AddRoadmapItemToSign { tx })
}

pub async fn submit_add_roadmap_item(
    algod: &Algod,
    signed: &AddRoadmapItemToSigned,
) -> Result<String> {
    let res = algod.broadcast_signed_transaction(&signed.tx).await?;
    log::debug!("Add roadmap item tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone)]
pub struct AddRoadmapItemToSign {
    pub tx: Transaction,
}

#[derive(Debug, Clone)]
pub struct AddRoadmapItemToSigned {
    pub tx: SignedTransaction,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RoadmapItemInputs {
    pub project_uuid: Uuid,
    pub title: String,
    pub parent: Box<Option<HashDigest>>,
}

impl RoadmapItemInputs {
    pub fn hash(&self) -> Result<HashDigest> {
        let bytes_to_hash = self.bytes_to_hash()?;
        let hashed = sha2::Sha512Trunc256::digest(&bytes_to_hash);
        Ok(HashDigest(hashed.into()))
    }

    fn bytes_to_hash(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }

    fn to_roadmap_item(&self) -> Result<RoadmapItem> {
        let hash = self.hash()?;
        Ok(RoadmapItem {
            project_uuid: self.project_uuid.clone(),
            title: self.title.clone(),
            parent: self.parent.clone(),
            hash,
        })
    }
}

// roadmap item model + hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapItem {
    pub project_uuid: Uuid,
    pub title: String,
    pub parent: Box<Option<HashDigest>>,
    pub hash: HashDigest,
}

fn roadmap_item_as_tx_note(item: &RoadmapItem) -> Result<Vec<u8>> {
    let base64 = BASE64.encode(&rmp_serde::to_vec_named(&item)?);
    let str = format!(
        "{}{}",
        project_uuid_note_prefix(&item.project_uuid),
        // encode the hashed item in the note
        // note that we've 2 rounds of msg pack:
        // 1) msg pack over the non-hashed object -> bytes to generate the hash (we could use any other binary representation)
        // 2) msg pack over the hashed object -> to store the serialized struct in note (we could use e.g. JSON instead, but msg pack is more compact)
        // to reverse: 1) deserialize hashed item from msg pack, 2) create non-hashed object with deserialized item, 3) validate non-hashed object with hash
        base64
    );
    // finally, encode the note as utf-8 (we could use any other byte representation)
    Ok(str.as_bytes().to_vec())
}
