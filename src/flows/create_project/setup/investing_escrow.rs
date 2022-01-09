use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{contract_account::ContractAccount, AcceptAsset, Pay, TransferAsset, TxnBuilder},
};
use anyhow::Result;
use serde::Serialize;

use crate::{
    flows::create_project::model::{
        SetupInvestEscrowSigned, SetupInvestingEscrowToSign, SubmitSetupEscrowRes,
    },
    teal::{render_template, TealSource, TealSourceTemplate},
};

#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;

/// The investing escrow holds the created project's assets (shares) to be bought by investors

pub async fn create_investing_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    asset_price: MicroAlgos,
    staking_escrow_address: &Address,
    source: &TealSourceTemplate,
) -> Result<ContractAccount> {
    render_and_compile_investing_escrow(
        algod,
        shares_asset_id,
        asset_price,
        staking_escrow_address,
        source,
    )
    .await
}

pub async fn render_and_compile_investing_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    asset_price: MicroAlgos,
    staking_escrow_address: &Address,
    source: &TealSourceTemplate,
) -> Result<ContractAccount> {
    let source =
        render_investing_escrow(source, shares_asset_id, asset_price, staking_escrow_address)?;
    Ok(ContractAccount::new(algod.compile_teal(&source.0).await?))
}

fn render_investing_escrow(
    source: &TealSourceTemplate,
    shares_asset_id: u64,
    asset_price: MicroAlgos, // affects the shares
    staking_escrow_address: &Address,
) -> Result<TealSource> {
    let escrow_source = render_template(
        source,
        EditTemplateContext {
            shares_asset_id: shares_asset_id.to_string(),
            asset_price: asset_price.to_string(),
            staking_escrow_address: staking_escrow_address.to_string(),
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
    asset_amount: u64,
    asset_price: MicroAlgos,
    creator: &Address,
    staking_escrow_address: &Address,
    params: &SuggestedTransactionParams,
) -> Result<SetupInvestingEscrowToSign> {
    log::debug!(
        "Setting up escrow with asset id: {}, amount: {}, creator: {:?}",
        shares_asset_id,
        asset_amount,
        creator
    );

    let escrow = create_investing_escrow(
        algod,
        shares_asset_id,
        asset_price,
        staking_escrow_address,
        source,
    )
    .await?;
    log::debug!("Generated investing escrow address: {:?}", escrow.address());

    // Send some funds to the escrow (min amount to hold asset, pay for opt in tx fee)
    let fund_algos_tx = &mut TxnBuilder::with(
        params.to_owned(),
        Pay::new(*creator, *escrow.address(), MicroAlgos(1_000_000)).build(),
    )
    .build();

    let shares_optin_tx = &mut TxnBuilder::with(
        params.to_owned(),
        AcceptAsset::new(*escrow.address(), shares_asset_id).build(),
    )
    .build();

    let fund_shares_asset_tx = &mut TxnBuilder::with(
        params.to_owned(),
        TransferAsset::new(*creator, shares_asset_id, asset_amount, *escrow.address()).build(),
    )
    .build();

    // TODO is it possible and does it make sense to execute these atomically?,
    // "sc opts in to asset and I send funds to sc"
    // TxGroup::assign_group_id(vec![optin_tx, fund_tx])?;

    Ok(SetupInvestingEscrowToSign {
        escrow,
        escrow_shares_optin_tx: shares_optin_tx.clone(),
        escrow_funding_algos_tx: fund_algos_tx.clone(),
        escrow_funding_shares_asset_tx: fund_shares_asset_tx.clone(),
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
    asset_price: String,
    staking_escrow_address: String,
}

#[cfg(test)]
mod tests {
    use crate::{
        dependencies, flows::create_project::setup::investing_escrow::render_investing_escrow,
        teal::load_teal_template,
    };
    use algonaut::core::{Address, MicroAlgos};
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
            MicroAlgos(1_000_000),
            &Address::new([0; 32]),
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
            MicroAlgos(1_000_000),
            &Address::new([0; 32]),
        )?;

        // deps
        let algod = dependencies::algod_for_tests();

        let _ = algod.compile_teal(&source.0).await?;

        Ok(())
    }
}
