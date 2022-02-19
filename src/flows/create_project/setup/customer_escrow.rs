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
use crate::{
    funds::FundsAssetId,
    teal::{render_template, TealSource, TealSourceTemplate},
};

// TODO no constants
// 1 asset (funds asset)
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(200_000);

pub async fn setup_customer_escrow(
    algod: &Algod,
    project_creator: &Address,
    central_address: &Address,
    source: &TealSourceTemplate,
    params: &SuggestedTransactionParams,
    funds_asset_id: FundsAssetId,
) -> Result<SetupCustomerEscrowToSign> {
    let escrow = render_and_compile_customer_escrow(algod, central_address, source).await?;

    let optin_to_funds_asset_tx = TxnBuilder::with(
        &params,
        AcceptAsset::new(*escrow.address(), funds_asset_id.0).build(),
    )
    .build()?;

    let fund_min_balance_tx = create_payment_tx(
        project_creator,
        escrow.address(),
        MIN_BALANCE + params.fee.max(params.min_fee),
        params,
    )
    .await?;

    Ok(SetupCustomerEscrowToSign {
        optin_to_funds_asset_tx,
        fund_min_balance_tx,
        escrow,
    })
}

pub async fn render_and_compile_customer_escrow(
    algod: &Algod,
    central_address: &Address,
    source: &TealSourceTemplate,
) -> Result<ContractAccount> {
    let source = render_customer_escrow(central_address, source)?;
    Ok(ContractAccount::new(algod.compile_teal(&source.0).await?))
}

pub fn render_customer_escrow(
    central_address: &Address,
    source: &TealSourceTemplate,
) -> Result<TealSource> {
    let escrow_source = render_template(
        source,
        CustomerEscrowTemplateContext {
            central_address: central_address.to_string(),
        },
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("customer_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

// might not be needed: submitting the create project txs together
pub async fn submit_setup_customer_escrow(
    algod: &Algod,
    signed: &SetupCustomerEscrowSigned,
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
pub struct SetupCustomerEscrowToSign {
    pub optin_to_funds_asset_tx: Transaction,
    pub fund_min_balance_tx: Transaction,
    pub escrow: ContractAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCustomerEscrowSigned {
    pub fund_min_balance_tx: SignedTransaction,
}

#[derive(Serialize)]
struct CustomerEscrowTemplateContext {
    central_address: String,
}
