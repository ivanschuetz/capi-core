use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;

use crate::{flows::create_project::model::Project, hashable::Hashable};

use super::{load_project::TxId, note::project_to_note};

pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn save_project(
    algod: &Algod,
    creator: &Address,
    project: &Project,
) -> Result<SaveProjectToSign> {
    let params = algod.suggested_transaction_params().await?;

    let note = project_to_note(project)?;
    // log::debug!("Note bytes: {:?}", note.len());

    let tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params
        },
        Pay::new(*creator, *creator, MicroAlgos(0)).build(),
    )
    .note(note)
    .build();

    Ok(SaveProjectToSign {
        tx,
        project: project.to_owned(),
    })
}

impl Hashable for Project {}

pub async fn submit_save_project(algod: &Algod, signed: SaveProjectSigned) -> Result<TxId> {
    let res = algod.broadcast_signed_transaction(&signed.tx).await?;
    log::debug!("Save project tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProjectToSign {
    pub tx: Transaction,
    pub project: Project,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProjectSigned {
    pub tx: SignedTransaction,
}
