use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{account::ContractAccount, Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::Serialize;

use crate::teal::{render_template, TealSource, TealSourceTemplate};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn setup_central_escrow(
    algod: &Algod,
    project_creator: &Address,
    source: TealSourceTemplate,
) -> Result<SetupCentralEscrowToSign> {
    let source = render_central_escrow(source)?;
    let escrow = ContractAccount::new(algod.compile_teal(&source.0).await?);
    Ok(SetupCentralEscrowToSign {
        fund_min_balance_tx: create_payment_tx(
            algod,
            project_creator,
            &escrow.address,
            MIN_BALANCE + FIXED_FEE,
        )
        .await?,
        escrow,
    })
}

fn render_central_escrow(source: TealSourceTemplate) -> Result<TealSource> {
    let escrow_source = render_template(source, CentralEscrowTemplateContext {})?;
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
    println!("Payment tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

async fn create_payment_tx(
    algod: &Algod,
    sender: &Address,
    receiver: &Address,
    amount: MicroAlgos,
) -> Result<Transaction> {
    let params = algod.suggested_transaction_params().await?;
    let tx = &mut TxnBuilder::with(params, Pay::new(*sender, *receiver, amount).build()).build();
    Ok(tx.clone())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCentralEscrowToSign {
    pub fund_min_balance_tx: Transaction,
    pub escrow: ContractAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupCentralEscrowSigned {
    pub fund_min_balance_tx: SignedTransaction,
}

#[derive(Serialize)]
struct CentralEscrowTemplateContext {}
