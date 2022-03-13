use crate::{
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    date_util::timestamp_seconds_to_date,
    flows::create_dao::{create_dao::Escrows, storage::note::base64_note_to_dao},
    queries::my_daos::StoredDao,
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

pub async fn load_dao(
    algod: &Algod,
    indexer: &Indexer,
    dao_id: &DaoId,
    escrows: &Escrows,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<StoredDao> {
    log::debug!("Fetching dao with tx id: {:?}", dao_id);

    let response = indexer.transaction_info(&dao_id.0.to_string()).await?;

    let tx = response.transaction;

    if tx.payment_transaction.is_some() {
        // Unexpected because we just fetched by (a non empty) note prefix, so logically it should have a note
        let note = tx
            .note
            .clone()
            .ok_or_else(|| anyhow!("Unexpected: dao storage tx has no note: {:?}", tx))?;

        let dao = base64_note_to_dao(algod, escrows, &note, capi_deps).await?;

        let round_time = tx
            .round_time
            .ok_or_else(|| anyhow!("Unexpected: tx has no round time: {:?}", tx))?;

        // Sanity check
        if tx.id.parse::<TxId>()? != dao_id.0 {
            return Err(anyhow!(
                "Invalid state: tx returned by indexer has a different id to the one we queried"
            ));
        }

        Ok(StoredDao {
            id: dao_id.to_owned(),
            dao,
            stored_date: timestamp_seconds_to_date(round_time)?,
        })
    } else {
        // It can be worth inspecting these, as their purpose would be unclear.
        // if the dao was created with our UI (and it worked correctly), the txs will always be payments.
        Err(anyhow!(
            "Daos txs query returned a non-payment tx: {:?}",
            tx
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct DaoId(pub TxId);
impl FromStr for DaoId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DaoId(s.parse()?))
    }
}
impl ToString for DaoId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl DaoId {
    pub fn bytes(&self) -> &[u8] {
        &self.0 .0 .0
    }
}
impl From<HashDigest> for DaoId {
    fn from(digest: HashDigest) -> Self {
        DaoId(digest.into())
    }
}
impl TryFrom<&[u8]> for DaoId {
    type Error = anyhow::Error;
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        Ok(DaoId(slice.try_into()?))
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
