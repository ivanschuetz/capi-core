use crate::capi_asset::capi_app_id::CapiAppId;
use algonaut::{
    core::{Address, SuggestedTransactionParams},
    transaction::{builder::OptInApplication, Transaction, TxnBuilder},
};
use anyhow::Result;
use mbase::models::dao_app_id::DaoAppId;

pub fn optin_to_capi_app(
    params: &SuggestedTransactionParams,
    app_id: CapiAppId,
    address: Address,
) -> Result<Transaction> {
    optin_to_app(params, app_id.0, address)
}

pub fn optin_to_dao_app(
    params: &SuggestedTransactionParams,
    app_id: DaoAppId,
    address: Address,
) -> Result<Transaction> {
    optin_to_app(params, app_id.0, address)
}

pub fn optin_to_app(
    params: &SuggestedTransactionParams,
    app_id: u64,
    address: Address,
) -> Result<Transaction> {
    Ok(TxnBuilder::with(
        params,
        OptInApplication::new(address, app_id)
            .app_arguments(vec!["optin".as_bytes().to_vec()])
            .build(),
    )
    .build()?)
}
