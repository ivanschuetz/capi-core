use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::TxnFee, contract_account::ContractAccount, AcceptAsset, Pay, TransferAsset,
        TxnBuilder,
    },
};
use anyhow::Result;
use serde::Serialize;

use crate::{
    algo_helpers::calculate_total_fee,
    flows::create_project::{
        model::{SetupInvestEscrowSigned, SetupInvestingEscrowToSign, SubmitSetupEscrowRes},
        share_amount::ShareAmount,
    },
    funds::{FundsAmount, FundsAssetId},
    teal::{render_template, TealSource, TealSourceTemplate},
};

#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;

// TODO no constant?
// 1 asset (funds asset)
const MIN_BALANCE: MicroAlgos = MicroAlgos(200_000);

/// The investing escrow holds the created project's assets (shares) to be bought by investors

pub async fn create_investing_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    share_price: &FundsAmount,
    funds_asset_id: &FundsAssetId,
    locking_escrow_address: &Address,
    central_escrow_address: &Address,
    source: &TealSourceTemplate,
    central_app_id: u64,
) -> Result<ContractAccount> {
    render_and_compile_investing_escrow(
        algod,
        shares_asset_id,
        share_price,
        funds_asset_id,
        locking_escrow_address,
        central_escrow_address,
        source,
        central_app_id,
    )
    .await
}

pub async fn render_and_compile_investing_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    share_price: &FundsAmount,
    funds_asset_id: &FundsAssetId,
    locking_escrow_address: &Address,
    central_escrow_address: &Address,
    source: &TealSourceTemplate,
    central_app_id: u64,
) -> Result<ContractAccount> {
    let source = render_investing_escrow(
        source,
        shares_asset_id,
        share_price,
        funds_asset_id,
        locking_escrow_address,
        central_escrow_address,
        central_app_id,
    )?;
    Ok(ContractAccount::new(algod.compile_teal(&source.0).await?))
}

pub fn render_investing_escrow(
    source: &TealSourceTemplate,
    shares_asset_id: u64,
    share_price: &FundsAmount,
    funds_asset_id: &FundsAssetId,
    locking_escrow_address: &Address,
    central_escrow_address: &Address,
    central_app_id: u64,
) -> Result<TealSource> {
    let escrow_source = render_template(
        source,
        EditTemplateContext {
            shares_asset_id: shares_asset_id.to_string(),
            share_price: share_price.0.to_string(),
            funds_asset_id: funds_asset_id.0.to_string(),
            locking_escrow_address: locking_escrow_address.to_string(),
            central_escrow_address: central_escrow_address.to_string(),
            app_id: central_app_id.to_string(),
        },
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("investing_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

#[allow(clippy::too_many_arguments)]
pub async fn setup_investing_escrow_txs(
    algod: &Algod,
    source: &TealSourceTemplate,
    shares_asset_id: u64,
    share_supply: ShareAmount,
    share_price: &FundsAmount,
    funds_asset_id: &FundsAssetId,
    creator: &Address,
    locking_escrow_address: &Address,
    central_escrow_address: &Address,
    params: &SuggestedTransactionParams,
    central_app_id: u64,
) -> Result<SetupInvestingEscrowToSign> {
    log::debug!(
        "Setting up investing escrow with asset id: {shares_asset_id}, transfer_share_amount: {share_supply}, creator: {creator}, locking_escrow_address: {locking_escrow_address}"
    );

    let escrow = create_investing_escrow(
        algod,
        shares_asset_id,
        share_price,
        funds_asset_id,
        locking_escrow_address,
        central_escrow_address,
        source,
        central_app_id,
    )
    .await?;
    log::debug!("Generated investing escrow address: {:?}", escrow.address());

    // Send min balance to the escrow
    let fund_algos_tx = &mut TxnBuilder::with(
        params,
        Pay::new(*creator, *escrow.address(), MIN_BALANCE).build(),
    )
    .build()?;

    let shares_optin_tx = &mut TxnBuilder::with_fee(
        params,
        TxnFee::zero(),
        AcceptAsset::new(*escrow.address(), shares_asset_id).build(),
    )
    .build()?;

    fund_algos_tx.fee = calculate_total_fee(&params, &[fund_algos_tx, shares_optin_tx])?;

    let transfer_shares_tx = &mut TxnBuilder::with(
        params,
        TransferAsset::new(
            *creator,
            shares_asset_id,
            share_supply.val(),
            *escrow.address(),
        )
        .build(),
    )
    .build()?;

    Ok(SetupInvestingEscrowToSign {
        escrow,
        escrow_shares_optin_tx: shares_optin_tx.clone(),
        escrow_funding_algos_tx: fund_algos_tx.clone(),
        escrow_funding_shares_asset_tx: transfer_shares_tx.clone(),
    })
}

// TODO submit these directly on create project submit?
pub async fn submit_investing_setup_escrow(
    algod: &Algod,
    signed: SetupInvestEscrowSigned,
) -> Result<SubmitSetupEscrowRes> {
    let shares_optin_escrow_res = algod
        .broadcast_signed_transaction(&signed.shares_optin_tx)
        .await?;
    log::debug!("shares_optin_escrow_res: {:?}", shares_optin_escrow_res);

    Ok(SubmitSetupEscrowRes {
        shares_optin_escrow_algos_tx_id: shares_optin_escrow_res.tx_id,
    })
}

#[derive(Serialize)]
struct EditTemplateContext {
    shares_asset_id: String,
    share_price: String,
    funds_asset_id: String,
    locking_escrow_address: String,
    central_escrow_address: String,
    app_id: String,
}

#[cfg(test)]
mod tests {
    use crate::{
        dependencies,
        flows::create_project::setup::investing_escrow::render_investing_escrow,
        funds::{FundsAmount, FundsAssetId},
        teal::load_teal_template,
    };
    use algonaut::core::Address;
    use anyhow::Result;
    use tokio::test;

    // Logs the rendered TEAL
    #[test]
    #[ignore]
    async fn test_render_escrow() -> Result<()> {
        let template = load_teal_template("investing_escrow")?;
        let source = render_investing_escrow(
            &template,
            123,
            &FundsAmount::new(1_000_000),
            &FundsAssetId(123),
            &Address::new([0; 32]),
            &Address::new([0; 32]),
            123,
        )?;
        let source_str = String::from_utf8(source.0)?;
        log::debug!("source: {}", source_str);
        Ok(())
    }

    #[test]
    #[ignore]
    async fn test_render_escrow_and_compile() -> Result<()> {
        let template = load_teal_template("investing_escrow")?;
        let source = render_investing_escrow(
            &template,
            123,
            &FundsAmount::new(1_000_000),
            &FundsAssetId(123),
            &Address::new([0; 32]),
            &Address::new([0; 32]),
            123,
        )?;

        // deps
        let algod = dependencies::algod_for_tests();

        let _ = algod.compile_teal(&source.0).await?;

        Ok(())
    }
}
