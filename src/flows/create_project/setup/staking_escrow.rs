use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        contract_account::ContractAccount, AcceptAsset, Pay, SignedTransaction, Transaction,
        TxnBuilder,
    },
};
use anyhow::Result;
use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::teal::{render_template, TealSource, TealSourceTemplate};

async fn create_staking_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    source: &TealSourceTemplate,
) -> Result<ContractAccount> {
    render_and_compile_staking_escrow(algod, shares_asset_id, source).await
}

pub async fn render_and_compile_staking_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    source: &TealSourceTemplate,
) -> Result<ContractAccount> {
    let source = render_staking_escrow(shares_asset_id, source)?;
    Ok(ContractAccount::new(algod.compile_teal(&source.0).await?))
}

fn render_staking_escrow(shares_asset_id: u64, source: &TealSourceTemplate) -> Result<TealSource> {
    let escrow_source = render_template(
        source,
        EditTemplateContext {
            shares_asset_id: shares_asset_id.to_string(),
        },
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("staking_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

pub async fn setup_staking_escrow_txs(
    algod: &Algod,
    source: &TealSourceTemplate,
    shares_asset_id: u64,
    creator: &Address,
    params: &SuggestedTransactionParams,
) -> Result<SetupStakingEscrowToSign> {
    log::debug!(
        "Setting up escrow with asset id: {}, creator: {:?}",
        shares_asset_id,
        creator
    );

    let escrow = create_staking_escrow(algod, shares_asset_id, source).await?;
    log::debug!("Generated staking escrow address: {:?}", *escrow.address());

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

    // TODO is it possible and does it make sense to execute these atomically?,
    // "sc opts in to asset and I send funds to sc"
    // TxGroup::assign_group_id(vec![optin_tx, fund_tx])?;

    Ok(SetupStakingEscrowToSign {
        escrow,
        escrow_shares_optin_tx: shares_optin_tx.clone(),
        escrow_funding_algos_tx: fund_algos_tx.clone(),
    })
}

pub async fn submit_staking_setup_escrow(
    algod: &Algod,
    signed: SetupStakingEscrowSigned,
) -> Result<SubmitSetupStakingEscrowRes> {
    let shares_optin_escrow_res = algod
        .broadcast_signed_transaction(&signed.shares_optin_tx)
        .await?;
    log::debug!("shares_optin_escrow_res: {:?}", shares_optin_escrow_res);

    Ok(SubmitSetupStakingEscrowRes {
        shares_optin_escrow_algos_tx_id: shares_optin_escrow_res.tx_id,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupStakingEscrowToSign {
    pub escrow: ContractAccount,
    pub escrow_shares_optin_tx: Transaction,
    // min amount to hold asset (shares) + asset optin tx fee
    pub escrow_funding_algos_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupStakingEscrowSigned {
    pub escrow: ContractAccount,
    pub shares_optin_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitSetupStakingEscrowRes {
    pub shares_optin_escrow_algos_tx_id: String,
}

#[derive(Serialize)]
struct EditTemplateContext {
    shares_asset_id: String,
}
