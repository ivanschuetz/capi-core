#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::{
    funds::FundsAssetId,
    teal::{render_template, TealSource, TealSourceTemplate},
};
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

// TODO no constants
// 1 asset (funds asset)
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(200_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn setup_central_escrow(
    algod: &Algod,
    project_creator: &Address,
    source: &TealSourceTemplate,
    params: &SuggestedTransactionParams,
    funds_asset_id: FundsAssetId,
) -> Result<SetupCentralEscrowToSign> {
    let escrow =
        render_and_compile_central_escrow(algod, project_creator, source, funds_asset_id).await?;

    let optin_to_funds_asset_tx = TxnBuilder::with(
        params.to_owned(),
        AcceptAsset::new(*escrow.address(), funds_asset_id.0).build(),
    )
    .build();

    let fund_min_balance_tx = create_payment_tx(
        project_creator,
        escrow.address(),
        MIN_BALANCE + FIXED_FEE,
        params,
    )
    .await?;

    Ok(SetupCentralEscrowToSign {
        optin_to_funds_asset_tx,
        fund_min_balance_tx,
        escrow,
    })
}

pub async fn render_and_compile_central_escrow(
    algod: &Algod,
    project_creator: &Address,
    source: &TealSourceTemplate,
    funds_asset_id: FundsAssetId,
) -> Result<ContractAccount> {
    let source = render_central_escrow(source, project_creator, funds_asset_id)?;
    Ok(ContractAccount::new(algod.compile_teal(&source.0).await?))
}

fn render_central_escrow(
    source: &TealSourceTemplate,
    project_creator: &Address,
    funds_asset_id: FundsAssetId,
) -> Result<TealSource> {
    let escrow_source = render_template(
        source,
        CentralEscrowTemplateContext {
            funds_asset_id: funds_asset_id.0.to_string(),
            project_creator_address: project_creator.to_string(),
        },
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("central_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

// might not be needed: submitting the create project txs together
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
    let tx = &mut TxnBuilder::with(
        params.to_owned(),
        Pay::new(*sender, *receiver, amount).build(),
    )
    .build();
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
    project_creator_address: String,
}

#[derive(Serialize)]
struct SomeContext {
    address: String,
}
