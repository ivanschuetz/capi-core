#[cfg(test)]
use crate::flows::create_project::model::Project;
#[cfg(test)]
use crate::flows::stake::logic::{stake, submit_stake, StakeSigned};
#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn stake_flow(
    algod: &Algod,
    project: &Project,
    investor: &Account,
    shares_to_stake: u64,
) -> Result<()> {
    let stake_to_sign = stake(
        &algod,
        investor.address(),
        shares_to_stake,
        project.shares_asset_id,
        project.central_app_id,
        &project.withdrawal_slot_ids,
        &project.staking_escrow,
    )
    .await?;

    let signed_app_call_tx = investor.sign_transaction(&stake_to_sign.central_app_call_setup_tx)?;
    let mut signed_slots_setup_txs = vec![];
    for slot_setup_tx in stake_to_sign.slot_setup_app_calls_txs {
        signed_slots_setup_txs.push(investor.sign_transaction(&slot_setup_tx)?);
    }

    let signed_shares_xfer_tx = investor.sign_transaction(&stake_to_sign.shares_xfer_tx)?;
    let tx_id = submit_stake(
        algod,
        StakeSigned {
            central_app_call_setup_tx: signed_app_call_tx,
            slot_setup_app_calls_txs: signed_slots_setup_txs,
            shares_xfer_tx_signed: signed_shares_xfer_tx,
        },
    )
    .await?;
    let _ = wait_for_pending_transaction(&algod, &tx_id);

    Ok(())
}
