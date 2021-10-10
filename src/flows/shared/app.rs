use algonaut::{
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{builder::OptInApplication, Transaction, TxnBuilder},
};
use anyhow::Result;

// TODO no constants
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn optin_to_app(
    params: &SuggestedTransactionParams,
    app_id: u64,
    address: Address,
) -> Result<Transaction> {
    Ok(TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        OptInApplication::new(address, app_id).build(),
    )
    .build())
}
