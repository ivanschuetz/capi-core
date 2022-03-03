use algonaut::{
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::CallApplication, Transaction, TxnBuilder},
};
use anyhow::Result;

pub async fn setup_app_tx(
    app_id: u64,
    creator: &Address,
    params: &SuggestedTransactionParams,
    central_escrow: &Address,
    customer_escrow: &Address,
) -> Result<Transaction> {
    log::debug!("Setting up app: {app_id}");
    let tx = TxnBuilder::with(
        params,
        CallApplication::new(*creator, app_id)
            .app_arguments(vec![central_escrow.0.to_vec(), customer_escrow.0.to_vec()])
            .build(),
    )
    .build()?;
    Ok(tx)
}
