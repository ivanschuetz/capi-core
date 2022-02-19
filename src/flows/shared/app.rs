use algonaut::{
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::OptInApplication, Transaction, TxnBuilder},
};
use anyhow::Result;

pub async fn optin_to_app(
    params: &SuggestedTransactionParams,
    app_id: u64,
    address: Address,
) -> Result<Transaction> {
    Ok(TxnBuilder::with(
        params.to_owned(),
        OptInApplication::new(address, app_id).build(),
    )
    .build())
}
