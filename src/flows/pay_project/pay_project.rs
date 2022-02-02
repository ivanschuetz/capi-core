use crate::flows::create_project::storage::load_project::TxId;
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{Pay, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn pay_project(
    algod: &Algod,
    customer: &Address,
    customer_escrow: &Address,
    amount: MicroAlgos,
) -> Result<PayProjectToSign> {
    let params = algod.suggested_transaction_params().await?;

    let tx = TxnBuilder::with(
        params.clone(),
        Pay::new(*customer, *customer_escrow, amount).build(),
    )
    .build();

    Ok(PayProjectToSign { tx })
}

pub async fn submit_pay_project(algod: &Algod, signed: PayProjectSigned) -> Result<TxId> {
    let res = algod.broadcast_signed_transaction(&signed.tx).await?;
    log::debug!("Pay project tx id: {:?}", res.tx_id);
    Ok(res.tx_id.parse()?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayProjectToSign {
    pub tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayProjectSigned {
    pub tx: SignedTransaction,
}
