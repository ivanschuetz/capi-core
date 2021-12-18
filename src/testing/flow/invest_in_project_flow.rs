#[cfg(test)]
use crate::flows::invest::app_optins::{
    invest_or_staking_app_optins_txs, submit_invest_or_staking_app_optins,
};
#[cfg(test)]
use crate::flows::{
    create_project::model::Project,
    invest::model::InvestResult,
    invest::{
        invest::{invest_txs, submit_invest},
        model::InvestSigned,
    },
};
#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
#[cfg(test)]
use anyhow::{anyhow, Result};

#[cfg(test)]
pub async fn invests_optins_flow(
    algod: &Algod,
    investor: &Account,
    project: &Project,
) -> Result<()> {
    // app optins (have to happen before invest_txs, which initializes investor's local state)
    let app_optins_txs =
        invest_or_staking_app_optins_txs(algod, project, &investor.address()).await?;

    // UI
    let mut app_optins_signed_txs = vec![];
    for optin_tx in app_optins_txs {
        app_optins_signed_txs.push(investor.sign_transaction(&optin_tx)?);
    }

    let app_optins_tx_id =
        submit_invest_or_staking_app_optins(algod, app_optins_signed_txs.clone()).await?;
    let _ = wait_for_pending_transaction(&algod, &app_optins_tx_id).await?;

    Ok(())
}

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
    let signed_central_app_setup_tx = investor.sign_transaction(&to_sign.central_app_setup_tx)?;
    let signed_shares_optin_tx = investor.sign_transaction(&to_sign.shares_asset_optin_tx)?;
    let signed_payment_tx = investor.sign_transaction(&to_sign.payment_tx)?;
    let signed_pay_escrow_fee_tx = investor.sign_transaction(&to_sign.pay_escrow_fee_tx)?;

    let invest_res = submit_invest(
        &algod,
        &InvestSigned {
            project: to_sign.project,
            central_app_setup_tx: signed_central_app_setup_tx,
            shares_asset_optin_tx: signed_shares_optin_tx,
            payment_tx: signed_payment_tx,
            pay_escrow_fee_tx: signed_pay_escrow_fee_tx,
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
        escrow_initial_amount,
        invest_res,
        project: project.to_owned(),
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
}
