use std::convert::TryInto;

use algonaut::crypto::HashDigest;
use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use serde::{de::DeserializeOwned, Serialize};

use crate::flows::create_project::storage::load_project::ProjectHash;

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

/// Prepended this to all the withdrawal notes, to filter txs
/// Has a fixed size of 12 characters (4 characters capi prefix + 8 characters withdraw string)
pub fn withdraw_note_prefix() -> String {
    format!("{}{}", capi_note_prefix(), "withdraw")
}

/// Base64 representation of the withdrawal prefix (utf8 encoding).
/// Used to query the withdrawal transactions from the indexer.
pub fn withdraw_note_prefix_base64() -> String {
    let str = withdraw_note_prefix();
    BASE64.encode(str.as_bytes())
}

/// Extract the note body
pub fn strip_withdraw_prefix_from_note(note: &[u8]) -> Result<String> {
    let note_decoded_bytes = &BASE64.decode(note)?;
    let note_str = std::str::from_utf8(note_decoded_bytes)?;

    Ok(note_str
        .strip_prefix(&withdraw_note_prefix())
        .ok_or_else(|| {
            anyhow!("Note (assumed to have been fetched with prefix) doesn't have expected prefix.")
        })?
        .to_owned())
}

fn project_hash_note_prefix(project_hash: &ProjectHash) -> Vec<u8> {
    [capi_note_prefix_bytes().as_slice(), &project_hash.0 .0].concat()
}

pub fn project_hash_note_prefix_base64(project_hash: &ProjectHash) -> String {
    let prefix = project_hash_note_prefix(project_hash);
    println!("prefix bytes: {:?}", prefix);
    BASE64.encode(&prefix)
}

// NOTE: the relationship between hash and obj is arbitrary - this represents just a specific note format.
// It can be e.g. the project's hash + roadmap item (the hash here acts as id: "item belongs to this project")
// Or it can be a hash of a derivation of the hashed object (e.g. we store a minimal representation of project, the hash belong to the original)
// Or it can be an actual hash of the object.
#[derive(Debug, Clone)]
pub struct ObjectAndHash<T>
where
    T: DeserializeOwned,
{
    pub hash: HashDigest,
    pub obj: T,
}

/// Extracts the hashed object from note
/// The note's expected format is: <CAPI PREFIX><HASH><OBJECT>.
/// Note that this does NOT verify the object against the hash
/// (Reason being that the hash might not be directly from the object, but from a derivation of it)
pub fn extract_hashed_object<T>(note: &str) -> Result<ObjectAndHash<T>>
where
    T: DeserializeOwned,
{
    // The api sends the bytes base64 encoded
    let note_decoded_bytes = BASE64.decode(note.as_bytes())?;

    extract_hash_and_object_from_decoded_note_bytes(&note_decoded_bytes)
}

// Just a helper function to prevent confusion with the non-decoded note string
fn extract_hash_and_object_from_decoded_note_bytes<T>(note: &[u8]) -> Result<ObjectAndHash<T>>
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

    Ok(ObjectAndHash { hash, obj: res })
}

pub trait AsNotePayload: Serialize {
    fn as_note_bytes(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }
}
