use super::{
    create_dao_specs::CreateDaoSpecs,
    model::{CreateDaoSigned, CreateDaoToSign, SubmitCreateDaoResult},
    storage::load_dao::DaoAppId,
};
use crate::{
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    flows::create_dao::{
        model::Dao,
        setup::{
            central_escrow::setup_central_escrow,
            customer_escrow::setup_customer_escrow,
            investing_escrow::setup_investing_escrow_txs,
            locking_escrow::setup_locking_escrow_txs,
            setup_app::{setup_app_tx, DaoInitData},
        },
    },
    funds::FundsAssetId,
    teal::{TealSource, TealSourceTemplate},
};
use algonaut::{algod::v2::Algod, core::Address, transaction::tx_group::TxGroup};
use anyhow::Result;

#[allow(clippy::too_many_arguments)]
pub async fn create_dao_txs(
    algod: &Algod,
    specs: &CreateDaoSpecs,
    creator: Address,
    owner: Address,
    shares_asset_id: u64,
    funds_asset_id: FundsAssetId,
    programs: &Programs,
    precision: u64,
    app_id: DaoAppId,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<CreateDaoToSign> {
    log::debug!(
        "Creating dao with specs: {:?}, shares_asset_id: {}, precision: {}",
        specs,
        shares_asset_id,
        precision
    );

    let params = algod.suggested_transaction_params().await?;

    let mut central_to_sign = setup_central_escrow(
        algod,
        &creator,
        &owner,
        &programs.escrows.central_escrow,
        &params,
        funds_asset_id,
        app_id,
    )
    .await?;

    let mut customer_to_sign = setup_customer_escrow(
        algod,
        &creator,
        central_to_sign.escrow.address(),
        &programs.escrows.customer_escrow,
        &params,
        funds_asset_id,
        &capi_deps.escrow,
        app_id,
    )
    .await?;

    let mut setup_locking_escrow_to_sign = setup_locking_escrow_txs(
        algod,
        &programs.escrows.locking_escrow,
        shares_asset_id,
        &creator,
        &params,
        app_id,
    )
    .await?;
    let mut setup_invest_escrow_to_sign = setup_investing_escrow_txs(
        algod,
        &programs.escrows.invest_escrow,
        shares_asset_id,
        specs.shares.supply,
        &specs.share_price,
        &funds_asset_id,
        &creator,
        setup_locking_escrow_to_sign.escrow.address(),
        central_to_sign.escrow.address(),
        &params,
        app_id,
    )
    .await?;

    let mut setup_app_tx = setup_app_tx(
        app_id,
        &creator,
        &params,
        &DaoInitData {
            central_escrow: *central_to_sign.escrow.address(),
            customer_escrow: *customer_to_sign.escrow.address(),
            investing_escrow: *setup_invest_escrow_to_sign.escrow.address(),
            locking_escrow: *setup_locking_escrow_to_sign.escrow.address(),
            shares_asset_id,
            funds_asset_id,
            project_name: specs.name.clone(),
            project_description: specs.description.clone(),
            share_price: specs.share_price,
            investors_part: specs.investors_part(),
            logo_url: specs.logo_url.clone(),
            social_media_url: specs.social_media_url.clone(),
            owner,
        },
    )
    .await?;

    TxGroup::assign_group_id(&mut [
        // setup app
        &mut setup_app_tx,
        // funding
        &mut central_to_sign.fund_min_balance_tx,
        &mut customer_to_sign.fund_min_balance_tx,
        &mut setup_locking_escrow_to_sign.escrow_funding_algos_tx,
        &mut setup_invest_escrow_to_sign.escrow_funding_algos_tx,
        // asset (shares) opt-ins
        &mut setup_locking_escrow_to_sign.escrow_shares_optin_tx,
        &mut setup_invest_escrow_to_sign.escrow_shares_optin_tx,
        // asset (funds asset) opt-ins
        &mut central_to_sign.optin_to_funds_asset_tx,
        &mut customer_to_sign.optin_to_funds_asset_tx,
        // asset (shares) transfer to investing escrow
        &mut setup_invest_escrow_to_sign.escrow_funding_shares_asset_tx,
    ])?;

    let locking_escrow = setup_locking_escrow_to_sign.escrow.clone();
    let locking_escrow_shares_optin_tx_signed =
        locking_escrow.sign(&setup_locking_escrow_to_sign.escrow_shares_optin_tx, vec![])?;
    let invest_escrow = setup_invest_escrow_to_sign.escrow.clone();
    let invest_escrow_shares_optin_tx_signed =
        invest_escrow.sign(&setup_invest_escrow_to_sign.escrow_shares_optin_tx, vec![])?;
    let central_escrow_optin_to_funds_asset_tx_signed = central_to_sign
        .escrow
        .sign(&central_to_sign.optin_to_funds_asset_tx, vec![])?;
    let customer_escrow_optin_to_funds_asset_tx_signed = customer_to_sign
        .escrow
        .sign(&customer_to_sign.optin_to_funds_asset_tx, vec![])?;
    let optin_txs = vec![
        locking_escrow_shares_optin_tx_signed,
        invest_escrow_shares_optin_tx_signed,
        central_escrow_optin_to_funds_asset_tx_signed,
        customer_escrow_optin_to_funds_asset_tx_signed,
    ];

    Ok(CreateDaoToSign {
        specs: specs.to_owned(),
        creator,

        setup_app_tx,

        locking_escrow: setup_locking_escrow_to_sign.escrow,
        invest_escrow: setup_invest_escrow_to_sign.escrow,
        central_escrow: central_to_sign.escrow,
        customer_escrow: customer_to_sign.escrow,

        escrow_funding_txs: vec![
            central_to_sign.fund_min_balance_tx,
            customer_to_sign.fund_min_balance_tx,
            setup_locking_escrow_to_sign.escrow_funding_algos_tx,
            setup_invest_escrow_to_sign.escrow_funding_algos_tx,
        ],
        optin_txs,

        xfer_shares_to_invest_escrow: setup_invest_escrow_to_sign.escrow_funding_shares_asset_tx,
    })
}

pub async fn submit_create_dao(
    algod: &Algod,
    signed: CreateDaoSigned,
) -> Result<SubmitCreateDaoResult> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);
    log::debug!(
        "Submitting, created dao specs: {:?}, creator: {:?}",
        signed.specs,
        signed.creator
    );

    let mut signed_txs = vec![signed.setup_app_tx];
    for tx in signed.escrow_funding_txs {
        signed_txs.push(tx)
    }
    for tx in signed.optin_txs {
        signed_txs.push(tx)
    }
    signed_txs.push(signed.xfer_shares_to_invest_escrow);

    // crate::teal::debug_teal_rendered(&signed_txs, "app_central_approval").unwrap();
    // crate::teal::debug_teal_rendered(&signed_txs, "investing_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&signed_txs, "central_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&signed_txs, "locking_escrow").unwrap();

    algod.broadcast_signed_transactions(&signed_txs).await?;

    Ok(SubmitCreateDaoResult {
        dao: Dao {
            specs: signed.specs,
            shares_asset_id: signed.shares_asset_id,
            funds_asset_id: signed.funds_asset_id,
            app_id: signed.app_id,
            invest_escrow: signed.invest_escrow,
            locking_escrow: signed.locking_escrow,
            customer_escrow: signed.customer_escrow,
            central_escrow: signed.central_escrow,
            creator: signed.creator,
        },
    })
}

#[derive(Debug)]
pub struct Programs {
    pub central_app_approval: TealSourceTemplate,
    pub central_app_clear: TealSource,
    pub escrows: Escrows,
}

#[derive(Debug)]
pub struct Escrows {
    pub central_escrow: TealSourceTemplate,
    pub customer_escrow: TealSourceTemplate,
    pub invest_escrow: TealSourceTemplate,
    pub locking_escrow: TealSourceTemplate,
}

/// TEAL related to the capi token
#[derive(Debug)]
pub struct CapiPrograms {
    pub app_approval: TealSourceTemplate,
    pub app_clear: TealSource,
    pub escrow: TealSourceTemplate,
}
