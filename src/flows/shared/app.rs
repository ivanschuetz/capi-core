use algonaut::{
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::OptInApplication, Transaction, TxnBuilder},
};
use anyhow::Result;
use mbase::models::dao_app_id::DaoAppId;

pub fn optin_to_dao_app(
    params: &SuggestedTransactionParams,
    app_id: DaoAppId,
    address: Address,
) -> Result<Transaction> {
    Ok(TxnBuilder::with(
        params,
        OptInApplication::new(address, app_id.0)
            .app_arguments(vec!["optin".as_bytes().to_vec()])
            .build(),
    )
    .build()?)
}
