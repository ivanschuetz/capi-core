use uuid::Uuid;

use crate::tx_note::project_uuid_note_prefix;

/// Generate the note for a withdrawal tx.
/// Note that we don't do anything with base64 here - we send directly the utf8 bytes.
/// base64 is needed when querying, because the indexer API requires it.
pub fn generate_withdrawal_tx_note(project_uuid: &Uuid, body: &str) -> Vec<u8> {
    format!("{}{}", project_uuid_note_prefix(project_uuid), body)
        .as_bytes()
        .to_vec()
}
