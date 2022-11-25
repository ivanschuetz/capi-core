use crate::{common_txs::pay, flows::create_dao::setup::setup_app::str_opt_def_to_bytes};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        builder::CallApplication, tx_group::TxGroup, SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::Result;
use mbase::{
    api::version::{versions_to_bytes, Versions},
    models::{dao_app_id::DaoAppId, share_amount::ShareAmount, tx_id::TxId},
    state::dao_app_state::{dao_global_state, Prospectus},
};
use serde::{Deserialize, Serialize};

/// Dao app data that is meant to be updated externally
#[derive(Debug, Clone)]
pub struct UpdatableDaoData {
    pub project_name: String,
    pub project_desc_url: Option<String>,

    pub image_url: Option<String>,
    pub social_media_url: String,

    pub prospectus: Option<Prospectus>,
    pub min_invest_shares: ShareAmount,
    pub max_invest_shares: ShareAmount,
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
    };

    let mut args = vec![
        "update_data".as_bytes().to_vec(),
        data.project_name.as_bytes().to_vec(),
        data.project_desc_url
            .as_ref()
            .map(|h| h.as_bytes().to_vec())
            .unwrap_or_default(),
        data.social_media_url.as_bytes().to_vec(),
        versions_to_bytes(versions)?,
        str_opt_def_to_bytes(data.prospectus.clone().map(|p| p.url)),
        str_opt_def_to_bytes(data.prospectus.clone().map(|p| p.hash)),
        data.min_invest_shares.val().to_be_bytes().to_vec(),
        data.max_invest_shares.val().to_be_bytes().to_vec(),
    ];

    if let Some(image_url) = &data.image_url {
        args.push(image_url.as_bytes().to_vec())
    }

    // We might make these updates more granular later. For now everything in 1 call.
    let mut update = TxnBuilder::with(
        &params,
        CallApplication::new(*owner, app_id.0)
            .app_arguments(args)
            .build(),
    )
    .build()?;

    // pay for optional image nft create tx
    update.fee = update.fee * 2;

    let increase_min_balance_tx = if data.image_url.is_some() {
        let mut pay_tx = pay(&params, owner, &app_id.address(), MicroAlgos(100_000))?;
        TxGroup::assign_group_id(&mut [&mut pay_tx, &mut update])?;
        Some(pay_tx)
    } else {
        None
    };

    Ok(UpdateAppToSign {
        update,
        increase_min_balance_tx,
    })
}

pub async fn submit_update_data(algod: &Algod, signed: UpdateDaoDataSigned) -> Result<TxId> {
    log::debug!("calling submit app data update..");
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let mut txs = vec![];
    if let Some(tx) = signed.increase_min_balance_tx {
        txs.push(tx)
    };
    txs.push(signed.update);

    // mbase::teal::debug_teal_rendered(&txs, "dao_app_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Unlock tx id: {:?}", res.tx_id);
    res.tx_id.parse()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateAppToSign {
    pub update: Transaction,
    pub increase_min_balance_tx: Option<Transaction>, // for possible image nft being created
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateDaoDataSigned {
    pub update: SignedTransaction,
    pub increase_min_balance_tx: Option<SignedTransaction>, // for possible image nft being created
}
