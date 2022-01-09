use crate::{
    flows::create_project::{
        create_project::Escrows,
        model::{CreateProjectSpecs, CreateSharesSpecs, Project},
        setup::{
            central_escrow::render_and_compile_central_escrow,
            customer_escrow::render_and_compile_customer_escrow,
            investing_escrow::render_and_compile_investing_escrow,
            staking_escrow::render_and_compile_staking_escrow,
        },
        storage::save_project::ProjectNoteProjectPayload,
    },
    hashable::Hashable,
    tx_note::{capi_note_prefix_bytes, extract_hashed_object},
};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    crypto::HashDigest,
    indexer::v2::Indexer,
    model::indexer::v2::{QueryTransaction, Role},
};
use anyhow::{anyhow, Error, Result};
use data_encoding::BASE64;
use futures::join;

fn project_hash_note_prefix(project_hash: &ProjectHash) -> Vec<u8> {
    [capi_note_prefix_bytes().as_slice(), &project_hash.0 .0].concat()
}

fn project_hash_note_prefix_base64(project_hash: &ProjectHash) -> String {
    let prefix = project_hash_note_prefix(project_hash);
    println!("prefix bytes: {:?}", prefix);
    BASE64.encode(&prefix)
}

pub async fn load_project(
    algod: &Algod,
    indexer: &Indexer,
    project_creator: &Address,
    project_hash: &ProjectHash,
    escrows: &Escrows,
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
            let hashed_stored_project = extract_hashed_object(&note)?;
            let stored_project = hashed_stored_project.obj;
            let stored_project_digest = ProjectHash(hashed_stored_project.hash);

            // double check that digest in note is what we sent in the query, (if not, exit with error - there's a bug and we shouldn't continue)
            if project_hash.0 != stored_project_digest.0 {
                return Err(anyhow!("Invalid state: The note prefix doesn't match the prefix used to query the indexer."));
            }

            let project = storable_project_to_project(
                algod,
                &stored_project,
                &stored_project_digest,
                escrows,
            )
            .await?;
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
pub struct ProjectHash(pub HashDigest);

/// Converts and completes the project data stored in note to a full project instance.
/// It also verifies the hash.
async fn storable_project_to_project(
    algod: &Algod,
    payload: &ProjectNoteProjectPayload,
    prefix_hash: &ProjectHash,
    escrows: &Escrows,
) -> Result<Project> {
    // Render and compile the escrows
    let central_escrow_account_fut = render_and_compile_central_escrow(
        algod,
        &payload.creator,
        &escrows.central_escrow,
        &payload.uuid,
    );
    let staking_escrow_account_fut =
        render_and_compile_staking_escrow(algod, payload.shares_asset_id, &escrows.staking_escrow);

    let (central_escrow_account_res, staking_escrow_account_res) =
        join!(central_escrow_account_fut, staking_escrow_account_fut);
    let central_escrow_account = central_escrow_account_res?;
    let staking_escrow_account = staking_escrow_account_res?;

    let customer_escrow_account_fut = render_and_compile_customer_escrow(
        algod,
        &central_escrow_account.address(),
        &escrows.customer_escrow,
    );

    let investing_escrow_account_fut = render_and_compile_investing_escrow(
        algod,
        payload.shares_asset_id,
        payload.asset_price,
        &staking_escrow_account.address(),
        &escrows.invest_escrow,
    );

    let (customer_escrow_account_res, investing_escrow_account_res) =
        join!(customer_escrow_account_fut, investing_escrow_account_fut);
    let customer_escrow_account = customer_escrow_account_res?;
    let investing_escrow_account = investing_escrow_account_res?;

    let project = Project {
        specs: CreateProjectSpecs {
            name: payload.name.clone(),
            shares: CreateSharesSpecs {
                token_name: payload.asset_name.clone(),
                count: payload.asset_supply,
            },
            asset_price: payload.asset_price,
            investors_share: payload.investors_share,
        },
        uuid: payload.uuid,
        creator: payload.creator,
        shares_asset_id: payload.shares_asset_id,
        central_app_id: payload.central_app_id,
        invest_escrow: investing_escrow_account,
        staking_escrow: staking_escrow_account,
        central_escrow: central_escrow_account,
        customer_escrow: customer_escrow_account,
    };

    // Verify hash (compare freshly calculated hash with prefix hash contained in note)
    let hash = ProjectHash(*project.hash()?.hash());
    if &hash != prefix_hash {
        return Err(anyhow!(
            "Hash verification failed: Note hash doesn't correspond to calculated hash"
        ));
    }

    Ok(project)
}
