use super::note::roadmap_item_to_note;
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    crypto::HashDigest,
    transaction::{Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use mbase::models::dao_id::DaoId;
use serde::{Deserialize, Serialize};
use sha2::Digest;

pub async fn add_roadmap_item(
    algod: &Algod,
    dao_creator: &Address,
    item_inputs: &RoadmapItemInputs,
) -> Result<AddRoadmapItemToSign> {
    let params = algod.suggested_transaction_params().await?;

    let roadmap_item = item_inputs.to_roadmap_item()?;
    let note = roadmap_item_to_note(&roadmap_item)?;

    // 0 payment to themselves - we use a minimal tx only to store data.
    let tx = TxnBuilder::with(
        &params,
        Pay::new(*dao_creator, *dao_creator, MicroAlgos(0)).build(),
    )
    .note(note)
    .build()?;

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
    pub dao_id: DaoId,
    pub title: String,
    pub parent: Box<Option<HashDigest>>,
    pub date: DateTime<Utc>,
}

impl RoadmapItemInputs {
    pub fn hash(&self) -> Result<HashDigest> {
        let bytes_to_hash = self.bytes_to_hash()?;
        let hashed = sha2::Sha512_256::digest(&bytes_to_hash);
        Ok(HashDigest(hashed.into()))
    }

    fn bytes_to_hash(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }

    fn to_roadmap_item(&self) -> Result<RoadmapItem> {
        let hash = self.hash()?;
        Ok(RoadmapItem {
            dao_id: self.dao_id,
            title: self.title.clone(),
            parent: self.parent.clone(),
            hash,
            date: self.date,
        })
    }
}

// roadmap item model + hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapItem {
    pub dao_id: DaoId,
    pub title: String,
    pub parent: Box<Option<HashDigest>>,
    pub hash: HashDigest,
    pub date: DateTime<Utc>,
}
