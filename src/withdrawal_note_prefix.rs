use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use uuid::Uuid;

/// Base64 representation of the withdrawal prefix (utf8 encoding).
/// Used to query the withdrawal transactions from the indexer.
pub fn withdrawal_tx_note_prefix_with_project_id_base64(project_uuid: &Uuid) -> String {
    let str = withdrawal_tx_note_prefix(project_uuid);
    BASE64.encode(str.as_bytes())
}

/// Generate the note for a withdrawal tx.
/// Note that we don't do anything with base64 here - we send directly the utf8 bytes.
/// base64 is needed when querying, because the indexer API requires it.
pub fn generate_withdrawal_tx_note(project_uuid: &Uuid, body: &str) -> Vec<u8> {
    format!("{}{}", withdrawal_tx_note_prefix(project_uuid), body)
        .as_bytes()
        .to_vec()
}

/// Extract the note body
pub fn strip_prefix_from_note(note: &[u8], project_uuid: &Uuid) -> Result<String> {
    let note_decoded_bytes = &BASE64.decode(note)?;
    let note_str = std::str::from_utf8(note_decoded_bytes)?;

    Ok(note_str
        .strip_prefix(&withdrawal_tx_note_prefix(project_uuid))
        .ok_or_else(|| {
            anyhow!("Note (assumed to have been fetched with prefix) doesn't have expected prefix.")
        })?
        .to_owned())
}

/// Global note prefix for all projects on the platform
fn capi_note_prefix() -> String {
    "capi".to_owned()
}

/// Prefix containing the project id
/// This is prepended this to all the withdrawal notes
fn withdrawal_tx_note_prefix(project_uuid: &Uuid) -> String {
    format!("{}{}", capi_note_prefix(), project_uuid)
}
