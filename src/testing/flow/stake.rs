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
        &project.staking_escrow,
    )
    .await?;

    let signed_app_call_tx = investor.sign_transaction(&stake_to_sign.app_call_tx)?;
    let signed_shares_xfer_tx = investor.sign_transaction(&stake_to_sign.shares_xfer_tx)?;
    let tx_id = submit_stake(
        algod,
        StakeSigned {
            app_call_tx: signed_app_call_tx,
            shares_xfer_tx_signed: signed_shares_xfer_tx,
        },
    )
    .await?;
    let _ = wait_for_pending_transaction(&algod, &tx_id);

    Ok(())
}
