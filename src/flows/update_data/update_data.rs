use crate::flows::create_dao::storage::load_dao::TxId;
use algonaut::{
    algod::v2::Algod,
    core::Address,
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use mbase::{
    api::version::{versions_to_bytes, VersionedAddress, Versions},
    models::{dao_app_id::DaoAppId, hash::GlobalStateHash},
    state::dao_app_state::dao_global_state,
};
use serde::{Deserialize, Serialize};

/// Dao app data that is meant to be updated externally
#[derive(Debug, Clone)]
pub struct UpdatableDaoData {
    pub customer_escrow: VersionedAddress,

    pub project_name: String,
    pub project_desc: Option<GlobalStateHash>,

    pub image_hash: Option<GlobalStateHash>,
    pub social_media_url: String,

    pub owner: Address,
}

pub async fn update_data(
    algod: &Algod,
    owner: &Address,
    app_id: DaoAppId,
    data: &UpdatableDaoData,
) -> Result<UpdateAppToSign> {
    let params = algod.suggested_transaction_params().await?;

    // fetch the fields that aren't updated manually, for the versions array.
    // we might optimize this, either by storing these separately or perhaps storing the versions in the same field as the addresses
    // consider also race conditions (loading state and someone updating it - though given only sender can submit probably not possible?)
    let current_state = dao_global_state(algod, app_id).await?;
    let versions = Versions {
        app_approval: current_state.app_approval_version,
        app_clear: current_state.app_clear_version,
        customer_escrow: data.customer_escrow.version,
    };

    // We might make these updates more granular later. For now everything in 1 call.
    let update = TxnBuilder::with(
        &params,
        CallApplication::new(*owner, app_id.0)
            .app_arguments(vec![
                "update_data".as_bytes().to_vec(),
                data.customer_escrow.address.0.to_vec(),
                data.project_name.as_bytes().to_vec(),
                data.project_desc
                    .as_ref()
                    .map(|h| h.bytes())
                    .unwrap_or_default(),
                data.image_hash
                    .as_ref()
                    .map(|h| h.bytes())
                    .unwrap_or_default(),
                data.social_media_url.as_bytes().to_vec(),
                data.owner.0.to_vec(),
                versions_to_bytes(versions)?,
            ])
            .build(),
    )
    .build()?;

    Ok(UpdateAppToSign { update })
}

pub async fn submit_update_data(algod: &Algod, signed: UpdateDaoDataSigned) -> Result<TxId> {
    log::debug!("calling submit app data update..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![signed.update];

    // mbase::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Unlock tx id: {:?}", res.tx_id);
    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAppToSign {
    pub update: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateDaoDataSigned {
    pub update: SignedTransaction,
}
