use crate::{
    flows::create_dao::storage::load_dao::DaoId, note::capi_note_prefix,
    roadmap::add_roadmap_item::RoadmapItem,
};
use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use std::convert::TryInto;

fn roadmap_note_identifier() -> [u8; 4] {
    // utf-8 encoding of "road"
    [63, 61, 70, 69]
}

pub fn roadmap_item_to_note(item: &RoadmapItem) -> Result<Vec<u8>> {
    let serialized = rmp_serde::to_vec_named(item)?;
    let version_bytes = u16::to_be_bytes(1);
    Ok([
        // add some prefixes to ensure that the context is correct
        // note that this makes sense specifically for the roadmap, as we're fetching transactions sent by the dao creator
        // the creator can unrestrictedly add roadmap items - security wise we only need to know the txs come from the creator
        // these prefixes are just helpers to indicate how to parse the data
        // note also that there's no strict reason to have 2 separate prefixes, it's a bit of an historic artifact since the capi prefix was used somewhere else too previously
        // but doesn't hurt to keep it - maybe we use again the capi prefix somewhere else.
        capi_note_prefix().as_slice(),
        &roadmap_note_identifier(),
        &version_bytes,
        &item.dao_id.bytes(),
        &serialized,
    ]
    .concat())
}

/// Parses a possible roadmap note.
/// If the note is not a roadmap note or belongs to a roadmap of a different dao, it returns Ok(None).
pub fn base64_maybe_roadmap_note_to_roadmap_item(
    note: &str,
    dao_id: DaoId,
) -> Result<Option<RoadmapItem>> {
    let bytes = BASE64.decode(note.as_bytes())?;
    maybe_roadmap_note_to_roadmap_item(&bytes, dao_id)
}

fn maybe_roadmap_note_to_roadmap_item(note: &[u8], dao_id: DaoId) -> Result<Option<RoadmapItem>> {
    if let Some(payload) = maybe_roadmap_note_to_roadmap_payload(note, dao_id)? {
        if payload.version != 1 {
            return Err(anyhow!(
                "Not supported roadmap item version in note: {}",
                payload.version
            ));
        }
        let item = rmp_serde::from_slice::<RoadmapItem>(&payload.variable)?;
        // Sanity check
        // Note that we're storing the dao id redundantly in the prefix and payload
        // This is not needed - just happened because the roadmap items contain a dao id (not sure this is actually needed)
        // and in the prefix is needed to allow querying the indexer by it (though this is not used currently,
        // as the Algorand indexer is unoptimized and these queries time out on Test/MainNet)
        // TODO (low prio) consider removing dao id from the items - leaving it only in prefix.
        if item.dao_id == dao_id {
            Ok(Some(item))
        } else {
            Err(anyhow!(
                "Invalid state: dao id in prefix doesn't match the payload dao id."
            ))
        }
    } else {
        Ok(None)
    }
}

fn maybe_roadmap_note_to_roadmap_payload(
    note: &[u8],
    dao_id: DaoId,
) -> Result<Option<RoadmapNotePayload>> {
    // Since we're parsing notes that are only potentially roadmap / capi notes / don't belong to the current dao,
    // not finding these prefixes is valid (it just returns None)
    if let Some(maybe_capi_prefix) = note.get(0..4) {
        if maybe_capi_prefix != capi_note_prefix() {
            return Ok(None);
        }
        if let Some(maybe_roadmap_prefix) = note.get(4..8) {
            if maybe_roadmap_prefix != roadmap_note_identifier() {
                return Ok(None);
            }
            let version_bytes = note.get(8..10).ok_or_else(|| {
                anyhow!("Not enough bytes in note to get version. Note: {note:?}")
            })?;
            let version = u16::from_be_bytes(version_bytes.try_into()?);

            if let Some(note_dao_id_bytes) = note.get(10..18) {
                let note_dao_id: DaoId = note_dao_id_bytes.try_into()?;
                if note_dao_id != dao_id {
                    return Ok(None);
                }

                let variable_bytes = note.get(18..note.len()).ok_or_else(|| {
                    anyhow!("Not enough bytes in note to get version. Note: {note:?}")
                })?;

                Ok(Some(RoadmapNotePayload {
                    version,
                    variable: variable_bytes.to_vec(),
                }))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone)]
struct RoadmapNotePayload {
    version: u16,
    variable: Vec<u8>,
}
