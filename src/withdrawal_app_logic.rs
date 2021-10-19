use algonaut::algod::v2::Algod;
use anyhow::Result;

use crate::withdrawal_app_state::withdrawal_amount_global_state;

pub async fn slot_is_free(algod: &Algod, slot_id: u64) -> Result<bool> {
    let slot_app = algod.application_information(slot_id).await?;
    let amount = withdrawal_amount_global_state(&slot_app);
    // never used (amount is None): it's free, reset (amount is 0): it's free
    Ok(amount.map(|a| a == 0).unwrap_or(true))
}
