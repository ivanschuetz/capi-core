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

#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::{
    algo_helpers::calculate_total_fee,
    funds::FundsAssetId,
    teal::{render_template_new, TealSource, TealSourceTemplate},
};

// TODO no constants
// 1 asset (funds asset)
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(200_000);

#[allow(clippy::too_many_arguments)]
pub async fn setup_customer_escrow(
    algod: &Algod,
    project_creator: &Address,
    central_address: &Address,
    source: &TealSourceTemplate,
    params: &SuggestedTransactionParams,
    funds_asset_id: FundsAssetId,
    capi_escrow_address: &Address,
    central_app_id: u64,
) -> Result<SetupCustomerEscrowToSign> {
    let escrow = render_and_compile_customer_escrow(
        algod,
        central_address,
        source,
        capi_escrow_address,
        central_app_id,
    )
    .await?;

    let mut optin_to_funds_asset_tx = TxnBuilder::with_fee(
        params,
        TxnFee::zero(),
        AcceptAsset::new(*escrow.address(), funds_asset_id.0).build(),
    )
    .build()?;

    let mut fund_min_balance_tx =
        create_payment_tx(project_creator, escrow.address(), MIN_BALANCE, params).await?;

    fund_min_balance_tx.fee = calculate_total_fee(
        params,
        &[&mut optin_to_funds_asset_tx, &mut fund_min_balance_tx],
    )?;

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
    capi_escrow_address: &Address,
    central_app_id: u64,
) -> Result<ContractAccount> {
    let source =
        render_customer_escrow(central_address, source, capi_escrow_address, central_app_id)?;
    Ok(ContractAccount::new(algod.compile_teal(&source.0).await?))
}

pub fn render_customer_escrow(
    central_address: &Address,
    source: &TealSourceTemplate,
    capi_escrow_address: &Address,
    central_app_id: u64,
) -> Result<TealSource> {
    let escrow_source = render_template_new(
        source,
        &[
            ("TMPL_CENTRAL_ESCROW_ADDRESS", &central_address.to_string()),
            ("TMPL_CAPI_ESCROW_ADDRESS", &capi_escrow_address.to_string()),
            ("TMPL_CENTRAL_APP_ID", &central_app_id.to_string()),
        ],
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
    capi_escrow_address: String,
    app_id: String,
}
