use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    flows::create_project::model::Project, hashable::Hashable, tx_note::capi_note_prefix_bytes,
};

use super::load_project::ProjectHash;

pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn save_project(
    algod: &Algod,
    creator: &Address,
    project: &Project,
) -> Result<SaveProjectToSign> {
    let params = algod.suggested_transaction_params().await?;

    let note = generate_note(project)?;

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
    pub hash: ProjectHash,
    pub project: Project,
}

fn generate_note(project: &Project) -> Result<ProjectNote> {
    let project_hash_result = project.hash()?;
    let project_hash = project_hash_result.hash();

    let capi_prefix_bytes: &[u8] = &capi_note_prefix_bytes();

    let project_note_payload: ProjectNoteProjectPayload = project.to_owned().into();
    let project_note_payload_bytes = project_note_payload.bytes()?;

    // Note that the hash belongs to the Project instance, not the saved payload.
    // This allows us to store a minimal representation and validate the generated full Project against the hash.
    // In this case minimal means that the programs (escrows) are not stored: they can be rendered on demand.
    let bytes = [
        capi_prefix_bytes,
        &project_hash.0,
        &project_note_payload_bytes,
    ]
    .concat();

    Ok(ProjectNote {
        bytes,
        hash: ProjectHash(project_hash.to_owned()),
    })
}

/// Bundles the note's bytes with some fields (assumed to be serialized in the bytes) for convenient access
#[derive(Debug, Clone)]
struct ProjectNote {
    bytes: Vec<u8>,

    hash: ProjectHash,
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
}

impl ProjectNoteProjectPayload {
    fn bytes(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }
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
        }
    }
}

impl Hashable for Project {}

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
