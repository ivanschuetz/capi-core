#[cfg(test)]
use crate::flows::create_project::model::Project;
#[cfg(test)]
use crate::flows::unstake::logic::unstake;
#[cfg(test)]
use crate::flows::unstake::logic::{submit_unstake, UnstakeSigned};
#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn unstake_flow(
    algod: &Algod,
    project: &Project,
    investor: &Account,
    shares_to_unstake: u64,
) -> Result<String> {
    let to_sign = unstake(
        &algod,
        investor.address(),
        shares_to_unstake,
        project.shares_asset_id,
        project.central_app_id,
        &project.withdrawal_slot_ids,
        &project.staking_escrow,
    )
    .await?;

    // UI
    let signed_central_app_optout = investor.sign_transaction(&to_sign.central_app_optout_tx)?;
    let mut signed_slots_setup_txs = vec![];
    for slot_optout_tx in to_sign.slot_optout_txs {
        signed_slots_setup_txs.push(investor.sign_transaction(&slot_optout_tx)?);
    }
    let signed_pay_xfer_fees = investor.sign_transaction(&to_sign.pay_shares_xfer_fee_tx)?;

    let tx_id = submit_unstake(
        algod,
        UnstakeSigned {
            central_app_optout_tx: signed_central_app_optout,
            slot_optout_txs: signed_slots_setup_txs,
            shares_xfer_tx_signed: to_sign.shares_xfer_tx,
            pay_shares_xfer_fee_tx: signed_pay_xfer_fees,
        },
    )
    .await?;

    Ok(tx_id)
}
