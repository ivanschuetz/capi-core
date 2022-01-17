use crate::flows::create_project::{
    create_project::Escrows, model::Project, storage::note::base64_note_to_project,
};
use algonaut::{algod::v2::Algod, crypto::HashDigest, indexer::v2::Indexer};
use anyhow::{anyhow, Result};
use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};
use std::{
    convert::{TryFrom, TryInto},
    str::FromStr,
};

pub async fn load_project(
    algod: &Algod,
    indexer: &Indexer,
    project_id: &ProjectId,
    escrows: &Escrows,
) -> Result<Project> {
    log::debug!("Fetching project with tx id: {:?}", project_id);

    let response = indexer.transaction_info(&project_id.0.to_string()).await?;

    let tx = response.transaction;

    if tx.payment_transaction.is_some() {
        // Unexpected because we just fetched by (a non empty) note prefix, so logically it should have a note
        let note = tx
            .note
            .clone()
            .ok_or_else(|| anyhow!("Unexpected: project storage tx has no note: {:?}", tx))?;

        let project = base64_note_to_project(algod, escrows, &note).await?;
        Ok(project)
    } else {
        // It can be worth inspecting these, as their purpose would be unclear.
        // if the project was created with our UI (and it worked correctly), the txs will always be payments.
        Err(anyhow!(
            "Projects txs query returned a non-payment tx: {:?}",
            tx
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
