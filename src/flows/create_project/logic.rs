use algonaut::{algod::v2::Algod, core::Address, transaction::tx_group::TxGroup};
use anyhow::{anyhow, Result};

use crate::{
    flows::create_project::{
        model::Project,
        setup::{
            create_app::create_app_tx, drain::setup_drain,
            investing_escrow::setup_investing_escrow_txs, staking_escrow::setup_staking_escrow_txs,
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

    // TODO reuse transaction params for all these txs, also in other places

    let mut drain_to_sign = setup_drain(
        algod,
        programs.central_escrow,
        programs.customer_escrow,
        &creator,
    )
    .await?;
    let create_app_tx = create_app_tx(
        algod,
        programs.central_app_approval,
        programs.central_app_clear,
        &creator,
        shares_asset_id,
        specs.shares.count,
        precision,
        specs.investors_share,
    )
    .await?;
    // let mut create_app_tx = create_app_tx(algod, &creator).await?;

    // TODO why do we do this (invest and staking escrows setup) here instead of directly on project creation? there seem to be no deps on post-creation things?
    let mut setup_staking_escrow_to_sign = setup_staking_escrow_txs(
        algod,
        programs.staking_escrow,
        shares_asset_id,
        specs.shares.count,
        &creator,
    )
    .await?;
    let mut setup_invest_escrow_to_sign = setup_investing_escrow_txs(
        algod,
        programs.invest_escrow,
        shares_asset_id,
        specs.shares.count,
        specs.asset_price,
        &creator,
        setup_staking_escrow_to_sign.escrow.address,
    )
    .await?;

    //////////////////////////////
    // asset opt-ins (have to be before the other transactions)
    TxGroup::assign_group_id(vec![
        &mut setup_staking_escrow_to_sign.escrow_shares_optin_tx,
        &mut setup_invest_escrow_to_sign.escrow_shares_optin_tx,
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

    //////////////////////////////

    TxGroup::assign_group_id(vec![
        &mut drain_to_sign.central.fund_min_balance_tx,
        &mut drain_to_sign.customer.fund_min_balance_tx,
        &mut setup_staking_escrow_to_sign.escrow_funding_algos_tx,
        &mut setup_invest_escrow_to_sign.escrow_funding_algos_tx,
    ])?;

    Ok(CreateProjectToSign {
        specs: specs.to_owned(),
        creator,

        staking_escrow: setup_staking_escrow_to_sign.escrow,
        invest_escrow: setup_invest_escrow_to_sign.escrow,
        central_escrow: drain_to_sign.central.escrow,
        customer_escrow: drain_to_sign.customer.escrow,

        // initial funding (algos) round, to be signed by creator
        escrow_funding_txs: vec![
            drain_to_sign.central.fund_min_balance_tx,
            drain_to_sign.customer.fund_min_balance_tx,
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
    log::debug!(
        "Submitting, created project specs: {:?}, creator: {:?}",
        signed.specs,
        signed.creator
    );

    log::debug!(
        "broadcasting escrow funding transactions({:?})",
        signed.escrow_funding_txs.len()
    );

    // crate::teal::debug_teal_rendered(&signed.optin_txs, "investing_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&signed.optin_txs, "staking_escrow").unwrap();

    let _ = algod
        .broadcast_signed_transactions(&signed.escrow_funding_txs)
        .await?;

    ///////////////////////////////////////////////////////////¯
    // TODO investigate: application_index is None in p_tx when executing the app create tx together with the other txs
    // see more notes in old repo
    ///////////////////////////////////////////////////////////¯
    log::debug!("Creating central app..");
    // let central_app_id = p_tx
    //     .application_index
    //     .ok_or(anyhow!("Pending tx didn't have app id"))?;
    let create_app_res = algod
        .broadcast_signed_transaction(&signed.create_app_tx)
        .await?;
    let p_tx = wait_for_pending_transaction(algod, &create_app_res.tx_id)
        .await?
        .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
    let central_app_id = p_tx
        .application_index
        .ok_or_else(|| anyhow!("Pending tx didn't have app id"))?;
    log::debug!("?? (see todo) central_app_id: {:?}", central_app_id);

    ///////////////////////////////////////////////////////////¯
    ///////////////////////////////////////////////////////////¯
    // Now that the escrows are funded, opt them in

    log::debug!(
        "broadcasting project creation opt ins({:?})",
        signed.optin_txs.len()
    );
    let submit_grouped_optin_txs_res = algod
        .broadcast_signed_transactions(&signed.optin_txs)
        .await?;
    let _ = wait_for_pending_transaction(algod, &submit_grouped_optin_txs_res.tx_id)
        .await?
        .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
    log::debug!("Executed optin txs");

    // now that the escrows are opted in, send them assets
    let submit_shares_xfer_tx_res = algod
        .broadcast_signed_transaction(&signed.xfer_shares_to_invest_escrow)
        .await?;
    let _ = wait_for_pending_transaction(algod, &submit_shares_xfer_tx_res.tx_id)
        .await?
        .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
    log::debug!("Executed escrow xfer txs");

    ///////////

    Ok(SubmitCreateProjectResult {
        project: Project {
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

pub struct Programs {
    pub central_app_approval: TealSourceTemplate,
    pub central_app_clear: TealSource,
    pub central_escrow: TealSourceTemplate,
    pub customer_escrow: TealSourceTemplate,
    pub invest_escrow: TealSourceTemplate,
    pub staking_escrow: TealSourceTemplate,
}
