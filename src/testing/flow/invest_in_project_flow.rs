#[cfg(test)]
use crate::flows::invest::app_optins::{
    invest_or_locking_app_optin_tx, submit_invest_or_locking_app_optin,
};
#[cfg(test)]
use crate::flows::{
    create_project::{model::Project, share_amount::ShareAmount, storage::load_project::ProjectId},
    invest::model::InvestResult,
    invest::{
        invest::{invest_txs, submit_invest},
        model::InvestSigned,
    },
};
#[cfg(test)]
use crate::funds::{FundsAmount, FundsAssetId};
#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use crate::state::account_state::funds_holdings;
#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::{anyhow, Result};

#[cfg(test)]
pub async fn invests_optins_flow(
    algod: &Algod,
    investor: &Account,
    project: &Project,
) -> Result<()> {
    // app optins (have to happen before invest_txs, which initializes investor's local state)
    let app_optin_tx = invest_or_locking_app_optin_tx(algod, project, &investor.address()).await?;

    // UI
    let app_optin_signed_tx = investor.sign_transaction(&app_optin_tx)?;

    let app_optin_tx_id =
        submit_invest_or_locking_app_optin(algod, app_optin_signed_tx.clone()).await?;
    let _ = wait_for_pending_transaction(&algod, &app_optin_tx_id).await?;

    Ok(())
}

// A user buys some shares
// Resets the network
#[cfg(test)]
pub async fn invests_flow(
    algod: &Algod,
    investor: &Account,
    buy_share_amount: ShareAmount,
    funds_asset_id: FundsAssetId,
    project: &Project,
    project_id: &ProjectId,
) -> Result<InvestInProjectTestFlowRes> {
    // remember initial investor's funds
    let investor_initial_amount =
        funds_holdings(algod, &investor.address(), funds_asset_id).await?;
    // remember initial central escrow's funds
    let central_escrow_initial_amount =
        funds_holdings(algod, project.central_escrow.address(), funds_asset_id).await?;

    let to_sign = invest_txs(
        &algod,
        &project,
        &investor.address(),
        &project.locking_escrow,
        project.central_app_id,
        project.shares_asset_id,
        buy_share_amount,
        funds_asset_id,
        project.specs.share_price,
        project_id,
    )
    .await?;

    // UI
    let signed_central_app_setup_tx = investor.sign_transaction(&to_sign.central_app_setup_tx)?;
    let signed_shares_optin_tx = investor.sign_transaction(&to_sign.shares_asset_optin_tx)?;
    let signed_payment_tx = investor.sign_transaction(&to_sign.payment_tx)?;

    let invest_res = submit_invest(
        &algod,
        &InvestSigned {
            project: to_sign.project,
            central_app_setup_tx: signed_central_app_setup_tx,
            shares_asset_optin_tx: signed_shares_optin_tx,
            payment_tx: signed_payment_tx,
            shares_xfer_tx: to_sign.shares_xfer_tx,
        },
    )
    .await?;

    // wait for tx to go through (so everything is on chain when returning to caller, e.g. to test)
    // TODO (low prio) should be in the tests rather?

    let _ = wait_for_pending_transaction(&algod, &invest_res.tx_id)
        .await?
        .ok_or(anyhow!("Couldn't get pending tx"))?;

    Ok(InvestInProjectTestFlowRes {
        investor_initial_amount,
        central_escrow_initial_amount,
        invest_res,
        project: project.to_owned(),
    })
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestInProjectTestFlowRes {
    pub investor_initial_amount: FundsAmount,
    pub central_escrow_initial_amount: FundsAmount,
    pub invest_res: InvestResult,
    pub project: Project,
}
