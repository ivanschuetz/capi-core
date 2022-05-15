use super::{
    model::{SetupDaoSigned, SetupDaoToSign, SubmitSetupDaoResult},
    setup_dao_specs::SetupDaoSpecs,
};
use crate::{
    api::version::VersionedTealSourceTemplate,
    capi_deps::CapiAssetDaoDeps,
    common_txs::pay,
    flows::create_dao::{
        model::Dao,
        setup::{
            customer_escrow::setup_customer_escrow,
            setup_app::{setup_app_tx, DaoInitData},
        },
    },
};
use algonaut::{
    algod::v2::Algod,
    core::{to_app_address, Address, MicroAlgos},
    transaction::{tx_group::TxGroup, TransferAsset, TxnBuilder},
};
use anyhow::Result;
use mbase::models::{dao_app_id::DaoAppId, funds::FundsAssetId};

#[allow(clippy::too_many_arguments)]
pub async fn setup_dao_txs(
    algod: &Algod,
    specs: &SetupDaoSpecs,
    creator: Address,
    owner: Address,
    shares_asset_id: u64,
    funds_asset_id: FundsAssetId,
    programs: &Programs,
    precision: u64,
    app_id: DaoAppId,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<SetupDaoToSign> {
    log::debug!(
        "Creating dao with specs: {:?}, shares_asset_id: {}, precision: {}",
        specs,
        shares_asset_id,
        precision
    );

    let params = algod.suggested_transaction_params().await?;

    let mut customer_to_sign = setup_customer_escrow(
        algod,
        &creator,
        &programs.escrows.customer_escrow,
        &params,
        funds_asset_id,
        &capi_deps.address,
        app_id,
    )
    .await?;

    // The non-investor shares currently just stay in the creator's wallet
    let mut transfer_shares_to_app_tx = TxnBuilder::with(
        &params,
        TransferAsset::new(
            creator,
            shares_asset_id,
            specs.shares_for_investors().val(),
            app_id.address(),
        )
        .build(),
    )
    .build()?;

    // TODO image hash

    let mut setup_app_tx = setup_app_tx(
        app_id,
        &creator,
        &params,
        &DaoInitData {
            customer_escrow: customer_to_sign.escrow.to_versioned_address(),
            app_approval_version: programs.central_app_approval.version,
            app_clear_version: programs.central_app_clear.version,
            shares_asset_id,
            funds_asset_id,
            project_name: specs.name.clone(),
            project_description: specs.description.clone(),
            share_price: specs.share_price,
            investors_share: specs.investors_share,
            image_hash: specs.image_hash.clone(),
            social_media_url: specs.social_media_url.clone(),
            owner,
            shares_for_investors: specs.shares_for_investors(),
        },
    )
    .await?;

    let app_address = to_app_address(app_id.0);
    // min balance to hold 2 assets (shares and funds asset)
    let mut fund_app_tx = pay(&params, &creator, &app_address, MicroAlgos(300_000))?;
    // pay the opt-in inner tx fees (shares and funds asset) (arbitrarily with this tx - could be any other in this group)
    fund_app_tx.fee = fund_app_tx.fee * 3;

    TxGroup::assign_group_id(&mut [
        &mut fund_app_tx,
        &mut setup_app_tx,
        &mut customer_to_sign.fund_min_balance_tx,
        &mut customer_to_sign.optin_to_funds_asset_tx,
        &mut transfer_shares_to_app_tx,
    ])?;

    let customer_escrow_optin_to_funds_asset_tx_signed = customer_to_sign
        .escrow
        .account
        .sign(customer_to_sign.optin_to_funds_asset_tx, vec![])?;

    Ok(SetupDaoToSign {
        specs: specs.to_owned(),
        creator,

        fund_app_tx,
        setup_app_tx,

        customer_escrow: customer_to_sign.escrow,

        customer_escrow_funding_tx: customer_to_sign.fund_min_balance_tx,
        customer_escrow_optin_to_funds_asset_tx: customer_escrow_optin_to_funds_asset_tx_signed,

        transfer_shares_to_app_tx,
    })
}

pub async fn submit_setup_dao(
    algod: &Algod,
    signed: SetupDaoSigned,
) -> Result<SubmitSetupDaoResult> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);
    log::debug!(
        "Submitting dao setup, specs: {:?}, creator: {:?}",
        signed.specs,
        signed.creator,
    );

    let signed_txs = vec![
        signed.app_funding_tx,
        signed.setup_app_tx,
        signed.fund_customer_escrow_tx,
        signed.customer_escrow_optin_to_funds_asset_tx,
        signed.transfer_shares_to_app_tx,
    ];

    // crate::dryrun_util::dryrun_all(algod, &signed_txs).await?;
    // crate::teal::debug_teal_rendered(&signed_txs, "dao_app_approval").unwrap();

    let tx_id = algod
        .broadcast_signed_transactions(&signed_txs)
        .await?
        .tx_id;

    Ok(SubmitSetupDaoResult {
        tx_id: tx_id.parse()?,
        dao: Dao {
            specs: signed.specs,
            shares_asset_id: signed.shares_asset_id,
            funds_asset_id: signed.funds_asset_id,
            app_id: signed.app_id,
            customer_escrow: signed.customer_escrow,
            owner: signed.creator,
        },
    })
}

#[derive(Debug)]
pub struct Programs {
    pub central_app_approval: VersionedTealSourceTemplate,
    pub central_app_clear: VersionedTealSourceTemplate,
    pub escrows: Escrows,
}

#[derive(Debug)]
pub struct Escrows {
    pub customer_escrow: VersionedTealSourceTemplate,
}

/// TEAL related to the capi token
#[derive(Debug)]
pub struct CapiPrograms {
    pub app_approval: VersionedTealSourceTemplate,
    pub app_clear: VersionedTealSourceTemplate,
}
