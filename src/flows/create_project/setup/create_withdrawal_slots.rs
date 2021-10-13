use crate::{
    flows::create_project::setup::create_withdrawal_slot::create_withdrawal_slot_tx,
    teal::{TealSource, TealSourceTemplate},
};
use algonaut::{algod::v2::Algod, core::Address, transaction::Transaction};
use anyhow::Result;

pub async fn create_withdrawal_slots_txs(
    algod: &Algod,
    count: u64,
    approval_source: TealSourceTemplate,
    clear_source: TealSource,
    creator: &Address,
    vote_threshold: u64,
) -> Result<Vec<Transaction>> {
    let mut slots = vec![];
    for i in 0..count {
        let slot = create_withdrawal_slot_tx(
            algod,
            approval_source.clone(),
            clear_source.clone(),
            &creator,
            vote_threshold,
            i,
        )
        .await?;
        slots.push(slot);
    }
    Ok(slots)
}
