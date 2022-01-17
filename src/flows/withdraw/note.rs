use super::withdraw::WithdrawalInputs;
use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use std::convert::TryInto;

// TODO write a test for save+load withdrawal(s) (like for project and roadmap items)

pub fn withdrawal_to_note(item: &WithdrawalInputs) -> Result<Vec<u8>> {
    let version_bytes = u16::to_be_bytes(1);
    // Consider compression for description, e.g. https://github.com/silentsokolov/rust-smaz
    // in a test it compressed ~40% of regular english text (from random wikipedia article)
    // it increased WASM file size by only ~16kb
    let description_bytes = item.description.as_bytes();
    Ok([version_bytes.as_slice(), description_bytes].concat())
}

pub fn base64_withdrawal_note_to_withdrawal_description(note: &str) -> Result<String> {
    let bytes = BASE64.decode(note.as_bytes())?;
    note_to_withdrawal_description(&bytes)
}

fn note_to_withdrawal_description(note: &[u8]) -> Result<String> {
    let payload = note_to_withdrawal_payload(note)?;
    if payload.version != 1 {
        return Err(anyhow!(
            "Invalid withdrawal item version: {}",
            payload.version
        ));
    }
    let description = std::str::from_utf8(&payload.variable)?;
    Ok(description.to_owned())
}

/// Note that we don't use prefixes here as the involved addresses (central escrow -> creator address)
/// (which we assume to have been used in the indexer query / result filtering)
/// are enough to identify withdrawals for a specific project.
fn note_to_withdrawal_payload(note: &[u8]) -> Result<WithdrawalPayload> {
    let version_bytes = note
        .get(0..2)
        .ok_or_else(|| anyhow!("Not enough bytes in note to get version. Note: {:?}", note))?;
    let version = u16::from_be_bytes(version_bytes.try_into()?);

    let variable_bytes = note
        .get(2..note.len())
        .ok_or_else(|| anyhow!("Not enough bytes in note to get version. Note: {:?}", note))?;

    Ok(WithdrawalPayload {
        version,
        variable: variable_bytes.to_vec(),
    })
}

#[derive(Debug, Clone)]
struct WithdrawalPayload {
    version: u16,
    variable: Vec<u8>,
}
