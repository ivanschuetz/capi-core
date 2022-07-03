use crate::flows::create_dao::model::Dao;
use algonaut::{algod::v2::Algod, crypto::HashDigest};
use anyhow::{anyhow, Result};
use data_encoding::BASE32_NOPAD;
use mbase::{
    models::{dao_id::DaoId, share_amount::ShareAmount},
    state::dao_app_state::dao_global_state,
};
use serde::{Deserialize, Serialize};
use std::{
    convert::{TryFrom, TryInto},
    hash::Hash,
    str::FromStr,
};

/// NOTE: this is an expensive function:
/// - Call to load dao app state
/// - Calls to retrieve TEAL templates for ALL the escrows (currently local, later this will come from API. Can be parallelized.)
/// - Call to retrieve asset information (supply etc, using the asset id stored in the app state)
/// - Calls to render and compile ALL the escrows (parallelized - 2 batches)
/// TODO parallelize more (and outside of this function, try to cache the dao, etc. to not have to call this often)
pub async fn load_dao(algod: &Algod, dao_id: DaoId) -> Result<Dao> {
    let app_id = dao_id.0;

    log::debug!("Fetching dao with id: {:?}", app_id);

    let dao_state = dao_global_state(algod, app_id).await?;

    // TODO store this state (redundantly in the same app field), to prevent this call?
    let asset_infos = algod.asset_information(dao_state.shares_asset_id).await?;

    let dao = Dao {
        funds_asset_id: dao_state.funds_asset_id,
        owner: dao_state.owner,
        shares_asset_id: dao_state.shares_asset_id,
        app_id,

        name: dao_state.project_name.clone(),
        descr_hash: dao_state.project_desc.clone(),
        token_name: asset_infos.params.name.unwrap_or_else(|| "".to_owned()),
        token_supply: ShareAmount::new(asset_infos.params.total),
        investors_share: dao_state.investors_share,
        share_price: dao_state.share_price,
        image_hash: dao_state.image_hash.clone(),
        image_nft: dao_state.image_nft.clone(),
        social_media_url: dao_state.social_media_url.clone(),
        raise_end_date: dao_state.min_funds_target_end_date,
        raise_min_target: dao_state.min_funds_target,
        raised: dao_state.raised,
    };

    Ok(dao)
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
