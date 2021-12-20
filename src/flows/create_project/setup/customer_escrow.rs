use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{account::ContractAccount, Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;
use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::teal::{render_template, TealSource, TealSourceTemplate};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn setup_customer_escrow(
    algod: &Algod,
    project_creator: &Address,
    central_address: Address,
    source: TealSourceTemplate,
    params: &SuggestedTransactionParams,
) -> Result<SetupCustomerEscrowToSign> {
    let source = render_customer_escrow(central_address, source)?;
    let escrow = ContractAccount::new(algod.compile_teal(&source.0).await?);
    Ok(SetupCustomerEscrowToSign {
        fund_min_balance_tx: create_payment_tx(
            project_creator,
            &escrow.address,
            MIN_BALANCE + FIXED_FEE,
            params,
        )
        .await?,
        escrow,
    })
}

fn render_customer_escrow(
    central_address: Address,
    source: TealSourceTemplate,
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
    println!("Payment tx id: {:?}", res.tx_id);
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
pub struct SetupCustomerEscrowToSign {
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
