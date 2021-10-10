#[cfg(test)]
use crate::flows::{
    create_project::model::Project,
    invest::model::InvestResult,
    invest::{
        logic::{invest_txs, submit_invest},
        model::InvestSigned,
    },
    shared::app::optin_to_app,
};
#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use algonaut::transaction::SignedTransaction;
#[cfg(test)]
use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
#[cfg(test)]
use anyhow::{anyhow, Result};

// A user buys some shares
// Resets the network
#[cfg(test)]
pub async fn invests_flow(
    algod: &Algod,
    investor: &Account,
    buy_asset_amount: u64,
    project: &Project,
) -> Result<InvestInProjectTestFlowRes> {
    // remember initial investor's funds
    let investor_infos = algod.account_information(&investor.address()).await?;
    let investor_initial_amount = investor_infos.amount;

    // remember initial escrow's funds
    let escrow_infos = algod
        .account_information(&project.invest_escrow.address)
        .await?;
    let escrow_initial_amount = escrow_infos.amount;

    // app optin (has to happen before invest_txs are submitted, which initializes the investor's local state)
    // note that this doesn't mean that it has to be executed before invest_txs, just before these txs are submitted
    let params = algod.suggested_transaction_params().await?;
    let app_optin_tx = optin_to_app(&params, project.central_app_id, investor.address()).await?;
    // UI
    let signed_app_optin_tx = investor.sign_transaction(&app_optin_tx)?;
    let res = algod
        .broadcast_signed_transaction(&signed_app_optin_tx)
        .await?;
    let _ = wait_for_pending_transaction(&algod, &res.tx_id).await?;

    let to_sign = invest_txs(
        &algod,
        &project,
        &investor.address(),
        &project.staking_escrow,
        project.central_app_id,
        project.shares_asset_id,
        buy_asset_amount,
        project.specs.asset_price,
    )
    .await?;

    // UI
    let signed_central_app_opt_in_tx = investor.sign_transaction(&to_sign.central_app_opt_in_tx)?;
    let signed_shares_optin_tx = investor.sign_transaction(&to_sign.shares_asset_optin_tx)?;
    let signed_payment_tx = investor.sign_transaction(&to_sign.payment_tx)?;
    let signed_pay_escrow_fee_tx = investor.sign_transaction(&to_sign.pay_escrow_fee_tx)?;

    let invest_res = submit_invest(
        &algod,
        &InvestSigned {
            project: to_sign.project,
            central_app_opt_in_tx: signed_central_app_opt_in_tx,
            shares_asset_optin_tx: signed_shares_optin_tx,
            payment_tx: signed_payment_tx,
            pay_escrow_fee_tx: signed_pay_escrow_fee_tx,
            shares_xfer_tx: to_sign.shares_xfer_tx,
            votes_xfer_tx: to_sign.votes_xfer_tx,
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
        escrow_initial_amount,
        invest_res,
        project: project.to_owned(),
        central_app_optin_tx: signed_app_optin_tx,
    })
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestInProjectTestFlowRes {
    pub investor_initial_amount: MicroAlgos,
    pub escrow_initial_amount: MicroAlgos,
    pub invest_res: InvestResult,
    pub project: Project,
    pub central_app_optin_tx: SignedTransaction,
}
