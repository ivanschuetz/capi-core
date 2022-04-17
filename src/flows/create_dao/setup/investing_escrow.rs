use crate::{
    algo_helpers::calculate_total_fee,
    api::version::{VersionedContractAccount, VersionedTealSourceTemplate},
    flows::create_dao::{
        model::{SetupInvestEscrowSigned, SetupInvestingEscrowToSign, SubmitSetupEscrowRes},
        share_amount::ShareAmount,
        storage::load_dao::DaoAppId,
    },
    funds::{FundsAmount, FundsAssetId},
    teal::{render_template_new, TealSource, TealSourceTemplate},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::TxnFee, contract_account::ContractAccount, AcceptAsset, Pay, TransferAsset,
        TxnBuilder,
    },
};
use anyhow::{anyhow, Result};
use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;

// TODO no constant?
// 1 asset (funds asset)
const MIN_BALANCE: MicroAlgos = MicroAlgos(200_000);

/// The investing escrow holds the created dao's assets (shares) to be bought by investors

#[allow(clippy::too_many_arguments)]
pub async fn create_investing_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    share_price: &FundsAmount,
    funds_asset_id: &FundsAssetId,
    locking_escrow_address: &Address,
    source: &VersionedTealSourceTemplate,
    app_id: DaoAppId,
) -> Result<VersionedContractAccount> {
    render_and_compile_investing_escrow(
        algod,
        shares_asset_id,
        share_price,
        funds_asset_id,
        locking_escrow_address,
        source,
        app_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn render_and_compile_investing_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    share_price: &FundsAmount,
    funds_asset_id: &FundsAssetId,
    locking_escrow_address: &Address,
    template: &VersionedTealSourceTemplate,
    app_id: DaoAppId,
) -> Result<VersionedContractAccount> {
    let source = match template.version.0 {
        1 => render_investing_escrow_v1(
            &template.template,
            shares_asset_id,
            share_price,
            funds_asset_id,
            locking_escrow_address,
            app_id,
        ),
        _ => Err(anyhow!(
            "Locking escrow version not supported: {:?}",
            template.version
        )),
    }?;

    Ok(VersionedContractAccount {
        version: template.version,
        account: ContractAccount::new(algod.compile_teal(&source.0).await?),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn render_investing_escrow_v1(
    source: &TealSourceTemplate,
    shares_asset_id: u64,
    share_price: &FundsAmount,
    funds_asset_id: &FundsAssetId,
    locking_escrow_address: &Address,
    app_id: DaoAppId,
) -> Result<TealSource> {
    let escrow_source = render_template_new(
        source,
        &[
            ("TMPL_SHARES_ASSET_ID", &shares_asset_id.to_string()),
            ("TMPL_SHARE_PRICE", &share_price.0.to_string()),
            ("TMPL_FUNDS_ASSET_ID", &funds_asset_id.0.to_string()),
            (
                "TMPL_LOCKING_ESCROW_ADDRESS",
                &locking_escrow_address.to_string(),
            ),
            ("TMPL_APP_ESCROW_ADDRESS", &app_id.address().to_string()),
            ("TMPL_CENTRAL_APP_ID", &app_id.0.to_string()),
        ],
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("investing_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

#[allow(clippy::too_many_arguments)]
pub async fn setup_investing_escrow_txs(
    algod: &Algod,
    source: &VersionedTealSourceTemplate,
    shares_asset_id: u64,
    share_supply: ShareAmount,
    share_price: &FundsAmount,
    funds_asset_id: &FundsAssetId,
    creator: &Address,
    locking_escrow_address: &Address,
    params: &SuggestedTransactionParams,
    app_id: DaoAppId,
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
        source,
        app_id,
    )
    .await?;
    log::debug!(
        "Generated investing escrow address: {:?}",
        escrow.account.address()
    );

    // Send min balance to the escrow
    let fund_algos_tx = &mut TxnBuilder::with(
        params,
        Pay::new(*creator, *escrow.account.address(), MIN_BALANCE).build(),
    )
    .build()?;

    let shares_optin_tx = &mut TxnBuilder::with_fee(
        params,
        TxnFee::zero(),
        AcceptAsset::new(*escrow.account.address(), shares_asset_id).build(),
    )
    .build()?;

    fund_algos_tx.fee = calculate_total_fee(params, &[fund_algos_tx, shares_optin_tx])?;

    let transfer_shares_tx = &mut TxnBuilder::with(
        params,
        TransferAsset::new(
            *creator,
            shares_asset_id,
            share_supply.val(),
            *escrow.account.address(),
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

// TODO submit these directly on create dao submit?
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
    app_id: String,
}

#[cfg(test)]
mod tests {
    use crate::{
        dependencies,
        flows::create_dao::{
            setup::investing_escrow::render_investing_escrow_v1, storage::load_dao::DaoAppId,
        },
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
        let source = render_investing_escrow_v1(
            &template,
            123,
            &FundsAmount::new(1_000_000),
            &FundsAssetId(123),
            &Address::new([0; 32]),
            DaoAppId(123),
        )?;
        let source_str = String::from_utf8(source.0)?;
        log::debug!("source: {}", source_str);
        Ok(())
    }

    #[test]
    #[ignore]
    async fn test_render_escrow_and_compile() -> Result<()> {
        let template = load_teal_template("investing_escrow")?;
        let source = render_investing_escrow_v1(
            &template,
            123,
            &FundsAmount::new(1_000_000),
            &FundsAssetId(123),
            &Address::new([0; 32]),
            DaoAppId(123),
        )?;

        // deps
        let algod = dependencies::algod_for_tests();

        let _ = algod.compile_teal(&source.0).await?;

        Ok(())
    }
}
