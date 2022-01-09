use std::convert::TryInto;

use algonaut::crypto::HashDigest;
use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

/// Global note prefix for all projects on the platform
/// fixed size of 4 characters
pub fn capi_note_prefix() -> String {
    "capi".to_owned()
}

// TODO use only this (remove everything text-based)
pub fn capi_note_prefix_bytes() -> [u8; 4] {
    // utf-8 encoding of "capi"
    [99, 97, 112, 105]
}

/// Prefix containing the project id
/// This is prepended this to all the withdrawal notes
/// Has a fixed size of 40 characters (4 characters capi prefix + 36 characters uuid)
pub fn project_uuid_note_prefix(project_uuid: &Uuid) -> String {
    format!("{}{}", capi_note_prefix(), project_uuid)
}

/// Base64 representation of the withdrawal prefix (utf8 encoding).
/// Used to query the withdrawal transactions from the indexer.
pub fn project_uuid_note_prefix_base64(project_uuid: &Uuid) -> String {
    let str = project_uuid_note_prefix(project_uuid);
    BASE64.encode(str.as_bytes())
}

/// Extract the note body
pub fn strip_prefix_from_note(note: &[u8], project_uuid: &Uuid) -> Result<String> {
    let note_decoded_bytes = &BASE64.decode(note)?;
    let note_str = std::str::from_utf8(note_decoded_bytes)?;

    Ok(note_str
        .strip_prefix(&project_uuid_note_prefix(project_uuid))
        .ok_or_else(|| {
            anyhow!("Note (assumed to have been fetched with prefix) doesn't have expected prefix.")
        })?
        .to_owned())
}

#[derive(Debug, Clone)]
pub struct HashedStoredObject<T>
where
    T: DeserializeOwned,
{
    // NOTE: this hash does NOT necessarily correspond directly to obj
    // it can belong to a derivation of it. E.g. for projects, we hash the full instance, which contains the escrow compiled programs
    // (we can't save them in the note, because of the size limitation)
    pub hash: HashDigest,
    pub obj: T,
}

/// Extracts the hashed object from note
/// The note's expected format is: <CAPI PREFIX><HASH><OBJECT>.
/// Note that this does NOT verify the object against the hash
/// (Reason being that the hash might not be directly from the object, but from a derivation of it)
pub fn extract_hashed_object<T>(note: &str) -> Result<HashedStoredObject<T>>
where
    T: DeserializeOwned,
{
    // The api sends the bytes base64 encoded
    let note_decoded_bytes = BASE64.decode(note.as_bytes())?;

    extract_hashed_object_from_decoded_note_bytes(&note_decoded_bytes)
}

// Just a helper function to prevent confusion with the non-decoded note string
fn extract_hashed_object_from_decoded_note_bytes<T>(note: &[u8]) -> Result<HashedStoredObject<T>>
where
    T: DeserializeOwned,
{
    let capi_prefix = note.get(0..4).ok_or_else(|| {
        anyhow!(
            "Not enough bytes in note to get capi prefix. Note: {:?}",
            note
        )
    })?;
    if capi_prefix != capi_note_prefix_bytes() {
        return Err(anyhow!(
            "Note's doesn't have the capi prefix. Found prefix: {:?}, note: {:?}",
            capi_prefix,
            note
        ));
    }

    let hash_bytes = note
        .get(4..36)
        .ok_or_else(|| anyhow!("Not enough bytes in note to get hash. Note: {:?}", note))?;
    let hash = HashDigest(hash_bytes.try_into()?);

    let hashed_obj = note.get(36..note.len()).ok_or_else(|| {
        anyhow!(
            "Not enough bytes in note to get hashed object. Note: {:?}",
            note
        )
    })?;

    let res = rmp_serde::from_slice(hashed_obj).map_err(|e| {
        anyhow!(
            "Failed deserializing hashed object bytes: {:?}, error: {}",
            hashed_obj,
            e
        )
    })?;

    Ok(HashedStoredObject { hash, obj: res })
}

pub trait AsNotePayload: Serialize {
    fn as_note_bytes(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }
}
