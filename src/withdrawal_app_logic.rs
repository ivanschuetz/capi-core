use algonaut::{algod::v2::Algod, core::Address};
use anyhow::Result;

use crate::withdrawal_app_state::{votes_local_state, withdrawal_amount_global_state};

pub async fn slot_is_free(algod: &Algod, slot_id: u64) -> Result<bool> {
    let slot_app = algod.application_information(slot_id).await?;
    let amount = withdrawal_amount_global_state(&slot_app);
    // never used (amount is None): it's free, reset (amount is 0): it's free
    Ok(amount.map(|a| a == 0).unwrap_or(true))
}

pub async fn voted(algod: &Algod, slot_id: u64, address: &Address) -> Result<bool> {
    let account = algod.account_information(address).await?;
    let votes = votes_local_state(&account.apps_local_state, slot_id).unwrap_or(0);
    Ok(votes > 0)
}
