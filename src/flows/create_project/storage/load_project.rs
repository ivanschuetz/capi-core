use std::{convert::TryInto, str::FromStr};

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
    tx_note::extract_hash_and_object_from_decoded_note,
};
use algonaut::{algod::v2::Algod, crypto::HashDigest, indexer::v2::Indexer};
use anyhow::{anyhow, Result};
use data_encoding::BASE32_NOPAD;
use futures::join;
use serde::{Deserialize, Serialize};

pub async fn load_project(
    algod: &Algod,
    indexer: &Indexer,
    project_id: &ProjectId,
    escrows: &Escrows,
) -> Result<Project> {
    log::debug!("Fetching project with tx id: {:?}", project_id);

    let response = indexer.transaction_info(&project_id.0.to_string()).await?;

    let tx = response.transaction;

    if tx.payment_transaction.is_some() {
        // Unexpected because we just fetched by (a non empty) note prefix, so logically it should have a note
        let note = tx
            .note
            .clone()
            .ok_or_else(|| anyhow!("Unexpected: project storage tx has no note: {:?}", tx))?;

        // The hash seems meaningless now that we're fetching the project data using the tx id (instead of the hash)
        // but we'll keep it for now. It doesn't hurt.
        let hashed_stored_project = extract_hash_and_object_from_decoded_note(&note)?;
        let stored_project = hashed_stored_project.obj;
        let stored_project_digest = hashed_stored_project.hash;

        let project =
            storable_project_to_project(algod, &stored_project, &stored_project_digest, escrows)
                .await?;

        Ok(project)
    } else {
        // It can be worth inspecting these, as their purpose would be unclear.
        // if the project was created with our UI (and it worked correctly), the txs will always be payments.
        Err(anyhow!(
            "Projects txs query returned a non-payment tx: {:?}",
            tx
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectId(pub TxId);
impl FromStr for ProjectId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ProjectId(s.parse()?))
    }
}
impl ToString for ProjectId {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}
impl ProjectId {
    pub fn bytes(&self) -> &[u8] {
        &self.0 .0 .0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
impl ToString for TxId {
    fn to_string(&self) -> String {
        BASE32_NOPAD.encode(&self.0 .0)
    }
}

/// Converts and completes the project data stored in note to a full project instance.
/// It also verifies the hash.
async fn storable_project_to_project(
    algod: &Algod,
    payload: &ProjectNoteProjectPayload,
    prefix_hash: &HashDigest,
    escrows: &Escrows,
) -> Result<Project> {
    // Render and compile the escrows
    let central_escrow_account_fut =
        render_and_compile_central_escrow(algod, &payload.creator, &escrows.central_escrow);
    let staking_escrow_account_fut =
        render_and_compile_staking_escrow(algod, payload.shares_asset_id, &escrows.staking_escrow);

    let (central_escrow_account_res, staking_escrow_account_res) =
        join!(central_escrow_account_fut, staking_escrow_account_fut);
    let central_escrow_account = central_escrow_account_res?;
    let staking_escrow_account = staking_escrow_account_res?;

    let customer_escrow_account_fut = render_and_compile_customer_escrow(
        algod,
        central_escrow_account.address(),
        &escrows.customer_escrow,
    );

    let investing_escrow_account_fut = render_and_compile_investing_escrow(
        algod,
        payload.shares_asset_id,
        payload.asset_price,
        staking_escrow_account.address(),
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
        creator: payload.creator,
        shares_asset_id: payload.shares_asset_id,
        central_app_id: payload.central_app_id,
        invest_escrow: investing_escrow_account,
        staking_escrow: staking_escrow_account,
        central_escrow: central_escrow_account,
        customer_escrow: customer_escrow_account,
    };

    // Verify hash (compare freshly calculated hash with prefix hash contained in note)
    // NOTE that this doesn't seem necessary anymore, as we're not using the prefix hash anymore to fetch (but the tx id instead)
    // but, why not: more verifications is better than less, if they don't impact significantly performance
    let hash = *project.compute_hash()?.hash();
    if &hash != prefix_hash {
        return Err(anyhow!(
            "Hash verification failed: Note hash doesn't correspond to calculated hash"
        ));
    }

    Ok(project)
}
