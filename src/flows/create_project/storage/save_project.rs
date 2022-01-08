use algonaut::{
    algod::v2::Algod,
    core::{Address, CompiledTeal, MicroAlgos, SuggestedTransactionParams},
    transaction::{Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{flows::create_project::model::Project, hashable::Hashable, tx_note::capi_note_prefix};

use super::load_project::ProjectNotePayloadHash;

pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn save_project(
    algod: &Algod,
    creator: &Address,
    project: &Project,
) -> Result<SaveProjectToSign> {
    let params = algod.suggested_transaction_params().await?;

    let project_note_payload: ProjectNoteProjectPayload = project.to_owned().into();
    let note = generate_note(project_note_payload)?;

    log::debug!("Note bytes: {:?}", note.bytes.len());

    let tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(*creator, *creator, MicroAlgos(0)).build(),
    )
    .note(note.bytes)
    .build();

    Ok(SaveProjectToSign {
        tx,
        stored_project: StoredProject {
            hash: note.hash,
            project: project.to_owned(),
        },
    })
}

/// Represents a project that has been already stored on the chain, with its hash
/// Already stored meaning: we submitted the storage tx successfully to the network (got a tx id)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredProject {
    pub hash: ProjectNotePayloadHash,
    pub project: Project,
}

fn generate_note(project_payload: ProjectNoteProjectPayload) -> Result<ProjectNote> {
    let hash_result = project_payload.hash()?;
    let hash = hash_result.hash();

    let capi_prefix = capi_note_prefix();
    let capi_prefix_bytes = capi_prefix.as_bytes();

    let bytes = [capi_prefix_bytes, &hash.0, &hash_result.hashed_bytes].concat();

    Ok(ProjectNote {
        bytes,
        hash: ProjectNotePayloadHash(hash.to_owned()),
        project_payload,
    })
}

/// Bundles the note's bytes with some fields (assumed to be serialized in the bytes) for convenient access
#[derive(Debug, Clone)]
struct ProjectNote {
    bytes: Vec<u8>,

    hash: ProjectNotePayloadHash,
    // the payload that was hashed - adding it just for readability
    #[allow(dead_code)]
    project_payload: ProjectNoteProjectPayload,
}

/// The project representation that's directly stored in the storage tx note
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectNoteProjectPayload {
    pub name: String,
    pub asset_id: u64,

    // NOTE: asset name and supply are redundant, we save them to not have to fetch the asset infos (also, they're short).
    // When someone shares their project, people should be able to see it as quickly as possible.
    // Note also that these asset properties are immutable (https://developer.algorand.org/docs/get-details/asa/#modifying-an-asset), so it's safe to save them.
    pub asset_name: String,
    pub asset_supply: u64,

    pub asset_price: MicroAlgos,
    pub investors_share: u64,
    pub uuid: Uuid,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub central_app_id: u64,
    pub invest_escrow: CompiledTeal,
    pub staking_escrow: CompiledTeal,
    pub central_escrow: CompiledTeal,
    pub customer_escrow: CompiledTeal,
}

impl From<Project> for ProjectNoteProjectPayload {
    fn from(p: Project) -> Self {
        ProjectNoteProjectPayload {
            name: p.specs.name.clone(),
            asset_id: p.shares_asset_id,
            asset_name: p.specs.shares.token_name,
            asset_supply: p.specs.shares.count,
            asset_price: p.specs.asset_price,
            investors_share: p.specs.investors_share,
            uuid: p.uuid,
            creator: p.creator,
            shares_asset_id: p.shares_asset_id,
            central_app_id: p.central_app_id,
            invest_escrow: p.invest_escrow.program,
            staking_escrow: p.staking_escrow.program,
            central_escrow: p.central_escrow.program,
            customer_escrow: p.customer_escrow.program,
        }
    }
}

impl Hashable for ProjectNoteProjectPayload {}

pub async fn submit_save_project(algod: &Algod, signed: SaveProjectSigned) -> Result<String> {
    let res = algod.broadcast_signed_transaction(&signed.tx).await?;
    log::debug!("Stake tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProjectToSign {
    pub tx: Transaction,
    pub stored_project: StoredProject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProjectSigned {
    pub tx: SignedTransaction,
}
