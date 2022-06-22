use super::{
    model::{SetupDaoSigned, SetupDaoToSign, SubmitSetupDaoResult},
    setup_dao_specs::SetupDaoSpecs,
};
use crate::{
    common_txs::pay,
    flows::create_dao::{
        model::Dao,
        setup::setup_app::{setup_app_tx, DaoInitData},
    },
};
use algonaut::{
    algod::v2::Algod,
    core::{to_app_address, Address, MicroAlgos},
    transaction::{tx_group::TxGroup, TransferAsset, TxnBuilder},
};
use anyhow::Result;
use mbase::{
    api::version::VersionedTealSourceTemplate,
    models::{dao_app_id::DaoAppId, funds::FundsAssetId},
};

#[allow(clippy::too_many_arguments)]
pub async fn setup_dao_txs(
    algod: &Algod,
    specs: &SetupDaoSpecs,
    creator: Address,
    shares_asset_id: u64,
    funds_asset_id: FundsAssetId,
    programs: &Programs,
    precision: u64,
    app_id: DaoAppId,
) -> Result<SetupDaoToSign> {
    log::debug!(
        "Creating dao with specs: {:?}, shares_asset_id: {}, precision: {}",
        specs,
        shares_asset_id,
        precision
    );

    let params = algod.suggested_transaction_params().await?;

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

    let mut setup_app_tx = setup_app_tx(
        app_id,
        &creator,
        &params,
        &DaoInitData {
            app_approval_version: programs.central_app_approval.version,
            app_clear_version: programs.central_app_clear.version,
            shares_asset_id,
            funds_asset_id,
            project_name: specs.name.clone(),
            descr_hash: specs.descr_hash.clone(),
            share_price: specs.share_price,
            investors_share: specs.investors_share,
            image_hash: specs.image_hash.clone(),
            social_media_url: specs.social_media_url.clone(),
            min_raise_target: specs.raise_min_target,
            min_raise_target_end_date: specs.raise_end_date,
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
        &mut transfer_shares_to_app_tx,
    ])?;

    Ok(SetupDaoToSign {
        specs: specs.to_owned(),
        creator,

        fund_app_tx,
        setup_app_tx,

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
        signed.transfer_shares_to_app_tx,
    ];

    // crate::dryrun_util::dryrun_all(algod, &signed_txs).await?;
    // mbase::teal::debug_teal_rendered(&signed_txs, "dao_app_approval").unwrap();

    let tx_id = algod
        .broadcast_signed_transactions(&signed_txs)
        .await?
        .tx_id;

    Ok(SubmitSetupDaoResult {
        tx_id: tx_id.parse()?,
        dao: Dao {
            shares_asset_id: signed.shares_asset_id,
            funds_asset_id: signed.funds_asset_id,
            app_id: signed.app_id,
            owner: signed.creator,

            name: signed.specs.name,
            descr_hash: signed.specs.descr_hash,
            token_name: signed.specs.shares.token_name,
            token_supply: signed.specs.shares.supply,
            investors_share: signed.specs.investors_share,
            share_price: signed.specs.share_price,
            image_hash: signed.specs.image_hash,
            social_media_url: signed.specs.social_media_url,
            raise_end_date: signed.specs.raise_end_date,
            raise_min_target: signed.specs.raise_min_target,
        },
    })
}

#[derive(Debug)]
pub struct Programs {
    pub central_app_approval: VersionedTealSourceTemplate,
    pub central_app_clear: VersionedTealSourceTemplate,
}

// TODO remove
/// TEAL related to the capi token
#[derive(Debug)]
pub struct CapiPrograms {
    pub app_approval: VersionedTealSourceTemplate,
    pub app_clear: VersionedTealSourceTemplate,
}
