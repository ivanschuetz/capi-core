use algonaut::{
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{Pay, Transaction, TxnBuilder},
};
use anyhow::Result;

pub fn pay(
    params: &SuggestedTransactionParams,
    sender: &Address,
    receiver: &Address,
    amount: MicroAlgos,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(&params, Pay::new(*sender, *receiver, amount).build()).build()?;
    Ok(tx)
}
