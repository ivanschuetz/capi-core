use crate::{
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    date_util::timestamp_seconds_to_date,
    flows::create_project::{create_project::Escrows, storage::note::base64_note_to_project},
    queries::my_projects::StoredProject,
};
use algonaut::{algod::v2::Algod, crypto::HashDigest, indexer::v2::Indexer};
use anyhow::{anyhow, Result};
use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};
use std::{
    convert::{TryFrom, TryInto},
    hash::Hash,
    str::FromStr,
};

pub async fn load_project(
    algod: &Algod,
    indexer: &Indexer,
    project_id: &ProjectId,
    escrows: &Escrows,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<StoredProject> {
    log::debug!("Fetching project with tx id: {:?}", project_id);

    let response = indexer.transaction_info(&project_id.0.to_string()).await?;

    let tx = response.transaction;

    if tx.payment_transaction.is_some() {
        // Unexpected because we just fetched by (a non empty) note prefix, so logically it should have a note
        let note = tx
            .note
            .clone()
            .ok_or_else(|| anyhow!("Unexpected: project storage tx has no note: {:?}", tx))?;

        let project = base64_note_to_project(algod, escrows, &note, capi_deps).await?;

        let round_time = tx
            .round_time
            .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

        // Sanity check
        if tx.id.parse::<TxId>()? != project_id.0 {
            return Err(anyhow!(
                "Invalid state: tx returned by indexer has a different id to the one we queried"
            ));
        }

        Ok(StoredProject {
            id: project_id.to_owned(),
            project,
            stored_date: timestamp_seconds_to_date(round_time)?,
        })
    } else {
        // It can be worth inspecting these, as their purpose would be unclear.
        // if the project was created with our UI (and it worked correctly), the txs will always be payments.
        Err(anyhow!(
            "Projects txs query returned a non-payment tx: {:?}",
            tx
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ProjectId(pub TxId);
impl FromStr for ProjectId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ProjectId(s.parse()?))
    }
}
impl ToString for ProjectId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl ProjectId {
    pub fn bytes(&self) -> &[u8] {
        &self.0 .0 .0
    }
}
impl From<HashDigest> for ProjectId {
    fn from(digest: HashDigest) -> Self {
        ProjectId(digest.into())
    }
}
impl TryFrom<&[u8]> for ProjectId {
    type Error = anyhow::Error;
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        Ok(ProjectId(slice.try_into()?))
    }
}

#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
pub struct TxId(pub HashDigest);
impl FromStr for TxId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes_vec = BASE32_NOPAD.decode(s.as_bytes())?;
        Ok(Self(HashDigest(bytes_vec.try_into().map_err(
            |v: Vec<u8>| anyhow!("Tx id bytes vec has wrong length: {}", v.len()),
        )?)))
    }
}

impl Hash for TxId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // HashDigest doesn't implement Hash - and we can't add it because it's in Algonaut,
        // and seems overkill to add this to Algonaut, so we call it on the wrapped bytes
        self.0 .0.hash(state);
    }
}

// Implemented to be consistent with the manual Hash implementation (also: Clippy complains otherwise)
// with the macro implementation it would compare the wrapped HashDigest instead of the bytes in HashDigest - it leads to the same result but not strictly consistent.
impl PartialEq for TxId {
    fn eq(&self, other: &Self) -> bool {
        self.0 .0 == other.0 .0
    }
}

impl ToString for TxId {
    fn to_string(&self) -> String {
        BASE32_NOPAD.encode(&self.0 .0)
    }
}
impl From<HashDigest> for TxId {
    fn from(digest: HashDigest) -> Self {
        TxId(digest)
    }
}
impl TryFrom<&[u8]> for TxId {
    type Error = anyhow::Error;
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        Ok(TxId(HashDigest(slice.try_into()?)))
    }
}
