use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use uuid::Uuid;

/// Global note prefix for all projects on the platform
/// fixed size of 4 characters
pub fn capi_note_prefix() -> String {
    "capi".to_owned()
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
