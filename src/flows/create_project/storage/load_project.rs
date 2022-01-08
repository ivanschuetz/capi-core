use crate::{
    flows::create_project::{
        model::{CreateProjectSpecs, CreateSharesSpecs, Project},
        storage::save_project::ProjectNoteProjectPayload,
    },
    hashable::Hashable,
    tx_note::{capi_note_prefix_bytes, extract_and_verify_hashed_object},
};
use algonaut::{
    core::Address,
    crypto::HashDigest,
    indexer::v2::Indexer,
    model::indexer::v2::{QueryTransaction, Role},
    transaction::contract_account::ContractAccount,
};
use anyhow::{anyhow, Error, Result};
use data_encoding::BASE64;

fn project_hash_note_prefix(project_hash: &ProjectNotePayloadHash) -> Vec<u8> {
    [capi_note_prefix_bytes().as_slice(), &project_hash.0 .0].concat()
}

fn project_hash_note_prefix_base64(project_hash: &ProjectNotePayloadHash) -> String {
    let prefix = project_hash_note_prefix(project_hash);
    println!("prefix bytes: {:?}", prefix);
    BASE64.encode(&prefix)
}

pub async fn load_project(
    indexer: &Indexer,
    project_creator: &Address,
    project_hash: &ProjectNotePayloadHash,
) -> Result<Project> {
    let note_prefix = project_hash_note_prefix_base64(project_hash);
    log::debug!(
        "Feching project with prefix: {:?}, sender: {:?}, hash (encoded in prefix): {:?}",
        note_prefix,
        project_creator,
        project_hash
    );

    let response = indexer
        .transactions(&QueryTransaction {
            // Note that querying by creator is not strictly necessary here (prefix with hash guarantees that we get the correct project data, it doesn't matter who submitted it)
            // but why not - if it doesn't slow down the query significantly (TODO check), more checks is always better.
            // It can help with (maybe unlikely - submitting significant amounts of txs can get expensive) possible flooding attacks (txs with the same project data), to slow down or cause OOM errors in the client.
            address: Some(project_creator.to_string()),
            address_role: Some(Role::Sender),
            note_prefix: Some(note_prefix.clone()),
            ..QueryTransaction::default()
        })
        .await?;

    // This early exit is not strictly needed, just for more understandable logs
    if response.transactions.is_empty() {
        return Err(anyhow!(
            "No project storage transactions found for prefix: {}",
            note_prefix
        ));
    }

    // Technically, there could be multiple results (most likely a bug, or something malicious, or used a different (buggy) frontend - at least the UUID should be different for each new project),
    // so we collect them and handle at the end
    let mut projects = vec![];

    // For now just a log warning. It should likely be a UI warning (TODO).
    if response.transactions.len() > 1 {
        log::warn!(
            "Multiple transactions found for (project hash: {:?}, creator: {})",
            project_hash,
            project_creator
        )
    }

    for tx in &response.transactions {
        if tx.payment_transaction.is_some() {
            let sender_address = tx.sender.parse::<Address>().map_err(Error::msg)?;

            // Sanity check
            if &sender_address != project_creator {
                return Err(anyhow!(
                    "Invalid state: tx sender isn't the sender we sent in the query"
                ));
            }

            // Unexpected because we just fetched by (a non empty) note prefix, so logically it should have a note
            let note = tx
                .note
                .clone()
                .ok_or_else(|| anyhow!("Unexpected: project storage tx has no note: {:?}", tx))?;

            // TODO extract the prefix (hash) and payload (project data)
            // let note_decoded_bytes = &BASE64.decode(note)?;

            // For now we'll fail the entire operation if the hash verification of one of the results fail
            // Considering that all these objects were created by the same account,
            // a failed hash verification means either malicious intent by the account, in which case it's suitable to invalidate other possible valid results created by them,
            // or a bug on our side, which would be critical and warrant to stop everything too.
            let hashed_stored_project = extract_and_verify_hashed_object(&note)?;
            let stored_project = hashed_stored_project.obj;
            let stored_project_digest = ProjectNotePayloadHash(hashed_stored_project.hash);

            // double check that digest in note is what we sent in the query, (if not, exit with error - there's a bug and we shouldn't continue)
            if project_hash.0 != stored_project_digest.0 {
                return Err(anyhow!("Invalid state: The note prefix doesn't match the prefix used to query the indexer."));
            }

            let project = storable_project_to_project(&stored_project, &stored_project_digest)?;
            projects.push(project);
        } else {
            // It can be worth inspecting these, as their purpose would be unclear.
            // if the project was created with our UI (and it worked correctly), the txs will always be payments.
            log::trace!("Projects txs query returned a non-payment tx: {:?}", tx);
        }
    }

    // We return the first project. Assumes:
    // - We just fetched projects by hash and validated them with the hash - so if there are multiple projects, they'd be indentical - we can return any.
    // - We already handled multiple results (i.e. transactions) for the hash.
    // Note that (as explained more in detail in other blocks above) multiple projects should be a rare scenario.
    if let Some(project) = projects.first() {
        Ok(project.to_owned())
    } else {
        Err(anyhow!(
            "No valid projects found for prefix: {}",
            note_prefix
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectNotePayloadHash(pub HashDigest);

fn storable_project_to_project(
    p: &ProjectNoteProjectPayload,
    prefix_hash: &ProjectNotePayloadHash,
) -> Result<Project> {
    // Security check: compare freshly calculated hash with prefix hash (used to find the project).
    // (Note: "prefix hash" is the prefix contained in the note)
    // This could happen because of a bug in our app or something malicious / a third party bug - anyone can store anything in the notes.
    // Side note: in this case we [assume to have] fetched using the project's creator as sender, meaning that if malicious, the creator is the attacker.
    let hash = ProjectNotePayloadHash(*p.hash()?.hash());
    if &hash != prefix_hash {
        return Err(anyhow!(
            "Invalid state: Stored project hash doesn't correspond to freshly calculated hash"
        ));
    }

    Ok(Project {
        specs: CreateProjectSpecs {
            name: p.name.clone(),
            shares: CreateSharesSpecs {
                token_name: p.asset_name.clone(),
                count: p.asset_supply,
            },
            asset_price: p.asset_price,
            investors_share: p.investors_share,
        },
        uuid: p.uuid,
        creator: p.creator,
        shares_asset_id: p.shares_asset_id,
        central_app_id: p.central_app_id,
        invest_escrow: ContractAccount::new(p.invest_escrow.clone()),
        staking_escrow: ContractAccount::new(p.staking_escrow.clone()),
        central_escrow: ContractAccount::new(p.central_escrow.clone()),
        customer_escrow: ContractAccount::new(p.customer_escrow.clone()),
    })
}
