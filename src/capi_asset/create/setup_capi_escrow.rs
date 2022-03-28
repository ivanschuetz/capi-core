#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::{
    algo_helpers::calculate_total_fee,
    api::version::{VersionedContractAccount, VersionedTealSourceTemplate},
    capi_asset::{capi_app_id::CapiAppId, capi_asset_id::CapiAssetId},
    flows::create_dao::storage::load_dao::TxId,
    funds::FundsAssetId,
    teal::{render_template_new, TealSource, TealSourceTemplate},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::TxnFee, contract_account::ContractAccount, tx_group::TxGroup, AcceptAsset, Pay,
        SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::{anyhow, Result};
use serde::Serialize;

// TODO no constants
// 2 assets (funds asset and capi asset)
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(300_000);

pub async fn setup_capi_escrow(
    algod: &Algod,
    min_balance_sender: &Address,
    source: &VersionedTealSourceTemplate,
    params: &SuggestedTransactionParams,
    capi_asset_id: CapiAssetId,
    funds_asset_id: FundsAssetId,
    app_id: CapiAppId,
) -> Result<SetupCentralEscrowToSign> {
    let escrow =
        render_and_compile_capi_escrow(algod, source, capi_asset_id, funds_asset_id, app_id)
            .await?;

    let fund_min_balance_tx = &mut create_payment_tx(
        min_balance_sender,
        escrow.account.address(),
        MIN_BALANCE,
        params,
    )
    .await?;

    let optin_to_capi_asset_tx = &mut TxnBuilder::with_fee(
        params,
        TxnFee::zero(),
        AcceptAsset::new(*escrow.account.address(), capi_asset_id.0).build(),
    )
    .build()?;

    let optin_to_funds_asset_tx = &mut TxnBuilder::with_fee(
        params,
        TxnFee::zero(),
        AcceptAsset::new(*escrow.account.address(), funds_asset_id.0).build(),
    )
    .build()?;

    fund_min_balance_tx.fee = calculate_total_fee(
        params,
        &[
            fund_min_balance_tx,
            optin_to_capi_asset_tx,
            optin_to_funds_asset_tx,
        ],
    )?;
    TxGroup::assign_group_id(&mut [
        fund_min_balance_tx,
        optin_to_capi_asset_tx,
        optin_to_funds_asset_tx,
    ])?;

    let optin_to_capi_asset_tx_signed = escrow.account.sign(optin_to_capi_asset_tx, vec![])?;
    let optin_to_funds_asset_tx_signed = escrow.account.sign(optin_to_funds_asset_tx, vec![])?;

    Ok(SetupCentralEscrowToSign {
        optin_to_capi_asset_tx: optin_to_capi_asset_tx_signed,
        optin_to_funds_asset_tx: optin_to_funds_asset_tx_signed,
        fund_min_balance_tx: fund_min_balance_tx.clone(),
        escrow,
    })
}

pub async fn render_and_compile_capi_escrow(
    algod: &Algod,
    template: &VersionedTealSourceTemplate,
    capi_asset_id: CapiAssetId,
    funds_asset_id: FundsAssetId,
    app_id: CapiAppId,
) -> Result<VersionedContractAccount> {
    let source = match template.version.0 {
        1 => render_capi_escrow_v1(&template.template, capi_asset_id, funds_asset_id, app_id),
        _ => Err(anyhow!(
            "Capi escrow version not supported: {:?}",
            template.version
        )),
    }?;

    Ok(VersionedContractAccount {
        version: template.version,
        account: ContractAccount::new(algod.compile_teal(&source.0).await?),
    })
}

fn render_capi_escrow_v1(
    source: &TealSourceTemplate,
    capi_asset_id: CapiAssetId,
    funds_asset_id: FundsAssetId,
    app_id: CapiAppId,
) -> Result<TealSource> {
    let escrow_source = render_template_new(
        source,
        &[
            ("TMPL_CAPI_ASSET_ID", &capi_asset_id.0.to_string()),
            ("TMPL_FUNDS_ASSET_ID", &funds_asset_id.0.to_string()),
            ("TMPL_CAPI_APP_ID", &app_id.0.to_string()),
        ],
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("capi_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

pub async fn submit_setup_capi_escrow(
    algod: &Algod,
    signed: &SetupCentralEscrowSigned,
) -> Result<TxId> {
    log::debug!("Will submit setup capi escrow..");
    let txs = vec![
        signed.fund_min_balance_tx.clone(),
        signed.optin_to_capi_asset_tx.clone(),
        signed.optin_to_funds_asset_tx.clone(),
    ];
    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Payment tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
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
    pub optin_to_capi_asset_tx: SignedTransaction,
    pub optin_to_funds_asset_tx: SignedTransaction,
    pub fund_min_balance_tx: Transaction,
    pub escrow: VersionedContractAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCentralEscrowSigned {
    pub optin_to_capi_asset_tx: SignedTransaction,
    pub optin_to_funds_asset_tx: SignedTransaction,
    pub fund_min_balance_tx: SignedTransaction,
}

#[derive(Serialize)]
struct CapiEscrowTemplateContext {
    capi_asset_id: String,
    funds_asset_id: String,
    app_id: String,
}

#[derive(Serialize)]
struct SomeContext {
    address: String,
}
