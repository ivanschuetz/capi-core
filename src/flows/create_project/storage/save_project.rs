use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::OptInApplication, tx_group::TxGroup, Pay, SignedTransaction, Transaction,
        TxnBuilder,
    },
};
use anyhow::Result;

use crate::{flows::create_project::model::Project, hashable::Hashable};

use super::{load_project::TxId, note::project_to_note};

pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

/// Actions to be executed directly after having created all the project's data (having a Project instance):
/// - Saving the (bundled) data: this gives us a mapping URL (tx id) -> project
/// - Opting in the creator to the app: This requires the app id (created in the previous step),
///   and is required by the next step (staking the shares, which accesses the app's local state)
pub async fn save_project_and_optin_to_app(
    algod: &Algod,
    creator: &Address,
    project: &Project,
) -> Result<SaveProjectToSign> {
    let params = algod.suggested_transaction_params().await?;

    let note = project_to_note(project)?;

    let mut app_optin_tx = TxnBuilder::with(
        params.clone(),
        OptInApplication::new(*creator, project.central_app_id)
            .app_arguments(vec!["opt_in_tmp".as_bytes().to_vec()])
            .build(),
    )
    .build();

    let mut save_project_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params
        },
        Pay::new(*creator, *creator, MicroAlgos(0)).build(),
    )
    .note(note)
    .build();

    TxGroup::assign_group_id(vec![&mut app_optin_tx, &mut save_project_tx])?;

    Ok(SaveProjectToSign {
        app_optin_tx,
        save_project_tx,
        project: project.to_owned(),
    })
}

impl Hashable for Project {}

pub async fn submit_save_project_and_optin_to_app(
    algod: &Algod,
    signed: SaveProjectSigned,
) -> Result<TxId> {
    let res = algod
        .broadcast_signed_transactions(&[signed.app_optin_tx, signed.save_project_tx])
        .await?;
    log::debug!("Save project tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProjectToSign {
    pub app_optin_tx: Transaction,
    pub save_project_tx: Transaction,
    pub project: Project,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveProjectSigned {
    pub app_optin_tx: SignedTransaction,
    pub save_project_tx: SignedTransaction,
}
