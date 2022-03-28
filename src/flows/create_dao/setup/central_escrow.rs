#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::{
    algo_helpers::calculate_total_fee,
    api::version::{VersionedContractAccount, VersionedTealSourceTemplate},
    flows::create_dao::storage::load_dao::DaoAppId,
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
use anyhow::{anyhow, Result};
use serde::Serialize;

// TODO no constants
// 1 asset (funds asset)
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(200_000);

pub async fn setup_central_escrow(
    algod: &Algod,
    funder: &Address,
    owner: &Address,
    source: &VersionedTealSourceTemplate,
    params: &SuggestedTransactionParams,
    funds_asset_id: FundsAssetId,
    app_id: DaoAppId,
) -> Result<SetupCentralEscrowToSign> {
    let escrow =
        render_and_compile_central_escrow(algod, source, owner, funds_asset_id, app_id).await?;

    let optin_to_funds_asset_tx = &mut TxnBuilder::with_fee(
        params,
        TxnFee::zero(),
        AcceptAsset::new(*escrow.account.address(), funds_asset_id.0).build(),
    )
    .build()?;

    let fund_min_balance_tx =
        &mut create_payment_tx(funder, escrow.account.address(), MIN_BALANCE, params).await?;

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
    template: &VersionedTealSourceTemplate,
    dao_creator: &Address,
    funds_asset_id: FundsAssetId,
    app_id: DaoAppId,
) -> Result<VersionedContractAccount> {
    let source = match template.version.0 {
        1 => render_central_escrow_v1(&template.template, dao_creator, funds_asset_id, app_id),
        _ => Err(anyhow!(
            "Central escrow version not supported: {:?}",
            template.version
        )),
    }?;

    Ok(VersionedContractAccount {
        version: template.version,
        account: ContractAccount::new(algod.compile_teal(&source.0).await?),
    })
}

fn render_central_escrow_v1(
    source: &TealSourceTemplate,
    owner: &Address,
    funds_asset_id: FundsAssetId,
    app_id: DaoAppId,
) -> Result<TealSource> {
    let escrow_source = render_template_new(
        source,
        &[
            ("TMPL_FUNDS_ASSET_ID", &funds_asset_id.0.to_string()),
            ("TMPL_OWNER", &owner.to_string()),
            ("TMPL_CENTRAL_APP_ID", &app_id.to_string()),
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
    pub escrow: VersionedContractAccount,
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
