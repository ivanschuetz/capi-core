use algonaut::{
    algod::v2::Algod,
    core::Address,
    transaction::{tx_group::TxGroup, SignedTransaction},
};
use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::{
    flows::create_project::{
        model::Project,
        setup::{
            central_escrow::setup_central_escrow, create_app::create_app_tx,
            customer_escrow::setup_customer_escrow, investing_escrow::setup_investing_escrow_txs,
            staking_escrow::setup_staking_escrow_txs,
        },
    },
    network_util::wait_for_pending_transaction,
    teal::{TealSource, TealSourceTemplate},
};

use super::model::{
    CreateProjectSigned, CreateProjectSpecs, CreateProjectToSign, SubmitCreateProjectResult,
};

pub async fn create_project_txs(
    algod: &Algod,
    specs: &CreateProjectSpecs,
    creator: Address,
    shares_asset_id: u64,
    programs: Programs,
    precision: u64,
) -> Result<CreateProjectToSign> {
    log::debug!(
        "Creating project with specs: {:?}, shares_asset_id: {}, precision: {}",
        specs,
        shares_asset_id,
        precision
    );

    let params = algod.suggested_transaction_params().await?;

    let project_uuid = Uuid::new_v4();

    let mut central_to_sign = setup_central_escrow(
        algod,
        &creator,
        programs.central_escrow,
        &params,
        &project_uuid,
    )
    .await?;

    let mut customer_to_sign = setup_customer_escrow(
        algod,
        &creator,
        central_to_sign.escrow.address(),
        programs.customer_escrow,
        &params,
    )
    .await?;

    let mut create_app_tx = create_app_tx(
        algod,
        programs.central_app_approval,
        programs.central_app_clear,
        &creator,
        shares_asset_id,
        specs.shares.count,
        precision,
        specs.investors_share,
        customer_to_sign.escrow.address(),
        central_to_sign.escrow.address(),
        &params,
    )
    .await?;

    // TODO why do we do this (invest and staking escrows setup) here instead of directly on project creation? there seem to be no deps on post-creation things?
    let mut setup_staking_escrow_to_sign = setup_staking_escrow_txs(
        algod,
        programs.staking_escrow,
        shares_asset_id,
        specs.shares.count,
        &creator,
        &params,
    )
    .await?;
    let mut setup_invest_escrow_to_sign = setup_investing_escrow_txs(
        algod,
        programs.invest_escrow,
        shares_asset_id,
        specs.shares.count,
        specs.asset_price,
        &creator,
        setup_staking_escrow_to_sign.escrow.address(),
        &params,
    )
    .await?;

    // First tx group to submit - everything except the asset (shares) xfer to the escrow (which requires opt-in to be submitted first)
    TxGroup::assign_group_id(vec![
        // app create (must be first in the group to return the app id, apparently)
        &mut create_app_tx,
        // funding
        &mut central_to_sign.fund_min_balance_tx,
        &mut customer_to_sign.fund_min_balance_tx,
        &mut setup_staking_escrow_to_sign.escrow_funding_algos_tx,
        &mut setup_invest_escrow_to_sign.escrow_funding_algos_tx,
        // asset (shares) opt-ins
        &mut setup_staking_escrow_to_sign.escrow_shares_optin_tx,
        &mut setup_invest_escrow_to_sign.escrow_shares_optin_tx,
        // asset (shares) transfer to investing escrow
        &mut setup_invest_escrow_to_sign.escrow_funding_shares_asset_tx,
    ])?;

    // Now that the lsig txs have been assigned a group id, sign (by their respective programs)
    let staking_escrow = setup_staking_escrow_to_sign.escrow.clone();
    let staking_escrow_shares_optin_tx_signed =
        staking_escrow.sign(&setup_staking_escrow_to_sign.escrow_shares_optin_tx, vec![])?;
    let invest_escrow = setup_invest_escrow_to_sign.escrow.clone();
    let invest_escrow_shares_optin_tx_signed =
        invest_escrow.sign(&setup_invest_escrow_to_sign.escrow_shares_optin_tx, vec![])?;
    let optin_txs = vec![
        staking_escrow_shares_optin_tx_signed,
        invest_escrow_shares_optin_tx_signed,
    ];

    Ok(CreateProjectToSign {
        uuid: project_uuid,
        specs: specs.to_owned(),
        creator,

        staking_escrow: setup_staking_escrow_to_sign.escrow,
        invest_escrow: setup_invest_escrow_to_sign.escrow,
        central_escrow: central_to_sign.escrow,
        customer_escrow: customer_to_sign.escrow,

        // initial funding (algos), to be signed by creator
        escrow_funding_txs: vec![
            central_to_sign.fund_min_balance_tx,
            customer_to_sign.fund_min_balance_tx,
            setup_staking_escrow_to_sign.escrow_funding_algos_tx,
            setup_invest_escrow_to_sign.escrow_funding_algos_tx,
        ],
        optin_txs,
        create_app_tx,

        // xfers to escrows: have to be executed after escrows are opted in
        xfer_shares_to_invest_escrow: setup_invest_escrow_to_sign.escrow_funding_shares_asset_tx,
    })
}

pub async fn submit_create_project(
    algod: &Algod,
    signed: CreateProjectSigned,
) -> Result<SubmitCreateProjectResult> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);
    log::debug!(
        "Submitting, created project specs: {:?}, creator: {:?}",
        signed.specs,
        signed.creator
    );
    log::debug!(
        "broadcasting escrow funding transactions({:?})",
        signed.escrow_funding_txs.len()
    );

    let mut signed_txs = vec![signed.create_app_tx];
    for tx in signed.escrow_funding_txs {
        signed_txs.push(tx)
    }
    for tx in signed.optin_txs {
        signed_txs.push(tx)
    }
    signed_txs.push(signed.xfer_shares_to_invest_escrow);

    // crate::teal::debug_teal_rendered(&signed_txs, "app_central_approval").unwrap();
    // crate::teal::debug_teal_rendered(&signed_txs, "investing_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&signed_txs, "staking_escrow").unwrap();

    let central_app_id = broadcast_txs_and_retrieve_app_id(algod, &signed_txs).await?;

    Ok(SubmitCreateProjectResult {
        project: Project {
            uuid: signed.uuid,
            specs: signed.specs,
            shares_asset_id: signed.shares_asset_id,
            central_app_id,
            invest_escrow: signed.invest_escrow,
            staking_escrow: signed.staking_escrow,
            customer_escrow: signed.customer_escrow,
            central_escrow: signed.central_escrow,
            creator: signed.creator,
        },
    })
}

async fn broadcast_txs_and_retrieve_app_id(
    algod: &Algod,
    txs: &[SignedTransaction],
) -> Result<u64> {
    log::debug!("Creating central app..");

    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "investing_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "staking_escrow").unwrap();

    let create_app_res = algod.broadcast_signed_transactions(txs).await?;
    let p_tx = wait_for_pending_transaction(algod, &create_app_res.tx_id)
        .await?
        .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
    let central_app_id = p_tx
        .application_index
        .ok_or_else(|| anyhow!("Pending tx didn't have app id"))?;

    Ok(central_app_id)
}

pub struct Programs {
    pub central_app_approval: TealSourceTemplate,
    pub central_app_clear: TealSource,
    pub central_escrow: TealSourceTemplate,
    pub customer_escrow: TealSourceTemplate,
    pub invest_escrow: TealSourceTemplate,
    pub staking_escrow: TealSourceTemplate,
}
