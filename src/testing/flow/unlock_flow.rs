#[cfg(test)]
use crate::flows::create_project::model::Project;
#[cfg(test)]
use crate::flows::create_project::{share_amount::ShareAmount, storage::load_project::TxId};
#[cfg(test)]
use crate::flows::unlock::unlock::unlock;
#[cfg(test)]
use crate::flows::unlock::unlock::{submit_unlock, UnlockSigned};
#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn unlock_flow(
    algod: &Algod,
    project: &Project,
    investor: &Account,
    shares_to_unlock: ShareAmount,
) -> Result<TxId> {
    let to_sign = unlock(
        &algod,
        investor.address(),
        shares_to_unlock,
        project.shares_asset_id,
        project.central_app_id,
        &project.locking_escrow,
    )
    .await?;

    // UI

    let signed_central_app_optout = investor.sign_transaction(&to_sign.central_app_optout_tx)?;

    let tx_id = submit_unlock(
        algod,
        UnlockSigned {
            central_app_optout_tx: signed_central_app_optout,
            shares_xfer_tx_signed: to_sign.shares_xfer_tx,
        },
    )
    .await?;

    Ok(tx_id)
}
