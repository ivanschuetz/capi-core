use crate::{
    flows::create_project::setup::create_withdrawal_slot::create_withdrawal_slot_tx,
    network_util::wait_for_pending_transaction,
    teal::{TealSource, TealSourceTemplate},
};
use algonaut::{
    algod::v2::Algod,
    core::Address,
    transaction::{SignedTransaction, Transaction},
};
use anyhow::{anyhow, Result};

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
            creator,
            vote_threshold,
            i,
        )
        .await?;
        slots.push(slot);
    }
    Ok(slots)
}

pub async fn submit_create_withdrawal_slots_txs(
    algod: &Algod,
    signed_txs: Vec<SignedTransaction>,
) -> Result<Vec<u64>> {
    log::debug!("Creating withdrawal slot apps..");
    let mut app_ids = vec![];
    for withdrawal_slot_app in &signed_txs {
        let create_app_res = algod
            .broadcast_signed_transaction(withdrawal_slot_app)
            .await?;
        let p_tx = wait_for_pending_transaction(algod, &create_app_res.tx_id)
            .await?
            .ok_or_else(|| anyhow!("Couldn't get pending tx"))?;
        let app_id = p_tx
            .application_index
            .ok_or_else(|| anyhow!("Pending tx didn't have app id"))?;
        log::debug!("Created withdrawal slot app id: {}", app_id);
        app_ids.push(app_id);
    }
    // Not really necessary (we exit if any of the requests creating the slot apps fails), just triple-check
    if app_ids.len() != signed_txs.len() {
        return Err(anyhow!("Couldn't create apps for all withdrawal slots"));
    }
    Ok(app_ids)
}
