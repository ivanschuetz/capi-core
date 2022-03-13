#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::{
    algo_helpers::calculate_total_fee,
    funds::FundsAssetId,
    teal::{render_template_new, TealSource, TealSourceTemplate},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::TxnFee, contract_account::ContractAccount, AcceptAsset, Pay, SignedTransaction,
        Transaction, TxnBuilder,
    },
};
use anyhow::Result;
use serde::Serialize;

// TODO no constants
// 1 asset (funds asset)
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(200_000);

pub async fn setup_central_escrow(
    algod: &Algod,
    dao_creator: &Address,
    source: &TealSourceTemplate,
    params: &SuggestedTransactionParams,
    funds_asset_id: FundsAssetId,
    central_app_id: u64,
) -> Result<SetupCentralEscrowToSign> {
    let escrow = render_and_compile_central_escrow(
        algod,
        dao_creator,
        source,
        funds_asset_id,
        central_app_id,
    )
    .await?;

    let optin_to_funds_asset_tx = &mut TxnBuilder::with_fee(
        params,
        TxnFee::zero(),
        AcceptAsset::new(*escrow.address(), funds_asset_id.0).build(),
    )
    .build()?;

    let fund_min_balance_tx =
        &mut create_payment_tx(dao_creator, escrow.address(), MIN_BALANCE, params).await?;

    fund_min_balance_tx.fee =
        calculate_total_fee(params, &[fund_min_balance_tx, optin_to_funds_asset_tx])?;

    Ok(SetupCentralEscrowToSign {
        optin_to_funds_asset_tx: optin_to_funds_asset_tx.clone(),
        fund_min_balance_tx: fund_min_balance_tx.clone(),
        escrow,
    })
}

pub async fn render_and_compile_central_escrow(
    algod: &Algod,
    dao_creator: &Address,
    source: &TealSourceTemplate,
    funds_asset_id: FundsAssetId,
    central_app_id: u64,
) -> Result<ContractAccount> {
    let source = render_central_escrow(source, dao_creator, funds_asset_id, central_app_id)?;
    Ok(ContractAccount::new(algod.compile_teal(&source.0).await?))
}

fn render_central_escrow(
    source: &TealSourceTemplate,
    dao_creator: &Address,
    funds_asset_id: FundsAssetId,
    central_app_id: u64,
) -> Result<TealSource> {
    let escrow_source = render_template_new(
        source,
        &[
            ("TMPL_FUNDS_ASSET_ID", &funds_asset_id.0.to_string()),
            ("TMPL_DAO_CREATOR", &dao_creator.to_string()),
            ("TMPL_CENTRAL_APP_ID", &central_app_id.to_string()),
        ],
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("central_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

// might not be needed: submitting the create dao txs together
pub async fn submit_setup_central_escrow(
    algod: &Algod,
    signed: &SetupCentralEscrowSigned,
) -> Result<String> {
    let res = algod
        .broadcast_signed_transaction(&signed.fund_min_balance_tx)
        .await?;
    log::debug!("Payment tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

async fn create_payment_tx(
    sender: &Address,
    receiver: &Address,
    amount: MicroAlgos,
    params: &SuggestedTransactionParams,
) -> Result<Transaction> {
    let tx = &mut TxnBuilder::with(params, Pay::new(*sender, *receiver, amount).build()).build()?;
    Ok(tx.clone())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCentralEscrowToSign {
    pub optin_to_funds_asset_tx: Transaction,
    pub fund_min_balance_tx: Transaction,
    pub escrow: ContractAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCentralEscrowSigned {
    pub fund_min_balance_tx: SignedTransaction,
}

#[derive(Serialize)]
struct CentralEscrowTemplateContext {
    funds_asset_id: String,
    dao_creator_address: String,
    app_id: String,
}

#[derive(Serialize)]
struct SomeContext {
    address: String,
}
