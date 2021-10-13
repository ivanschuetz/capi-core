#[cfg(test)]
use super::vote::vote_flow;
#[cfg(test)]
use crate::flows::{
    create_project::model::{CreateProjectSpecs, Project},
    withdraw::logic::{submit_withdraw, withdraw, WithdrawSigned},
};
#[cfg(test)]
use crate::{
    flows::shared::app::optin_to_app,
    network_util::wait_for_pending_transaction,
    testing::flow::{
        create_project::create_project_flow,
        customer_payment_and_drain_flow::{
            customer_payment_and_drain_flow, CustomerPaymentAndDrainFlowRes,
        },
    },
};
#[cfg(test)]
use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

/// Creates project, votes to meet threshold, drains an amount
#[cfg(test)]
pub async fn withdraw_precs(
    algod: &Algod,
    creator: &Account,
    specs: &CreateProjectSpecs,
    drainer: &Account,
    customer: &Account,
    voter: &Account,
    pay_and_drain_amount: MicroAlgos,
) -> Result<WithdrawTestPrecsRes> {
    let project = create_project_flow(&algod, &creator, &specs, 3).await?;
    // meet the voting threshold so the withdrawal is approved
    let _ = vote_flow(
        algod,
        voter,
        &project,
        project.specs.vote_threshold_units(), // enough units for vote to pass
    )
    .await?;

    // payment and draining
    let drain_res = customer_payment_and_drain_flow(
        &algod,
        &drainer,
        &customer,
        pay_and_drain_amount,
        &project,
    )
    .await?;
    let central_escrow_balance_after_drain = algod
        .account_information(&drain_res.project.central_escrow.address)
        .await?
        .amount;

    // app optin
    let params = algod.suggested_transaction_params().await?;
    let app_optin_tx = optin_to_app(&params, project.central_app_id, creator.address()).await?;
    // UI
    let signed_app_optin_tx = creator.sign_transaction(&app_optin_tx)?;
    let res = algod
        .broadcast_signed_transaction(&signed_app_optin_tx)
        .await?;
    let _ = wait_for_pending_transaction(&algod, &res.tx_id).await?;

    // end precs

    Ok(WithdrawTestPrecsRes {
        project,
        central_escrow_balance_after_drain,
        drain_res,
    })
}

#[cfg(test)]
pub async fn withdraw_flow(
    algod: &Algod,
    project: &Project,
    creator: &Account,
    amount: MicroAlgos,
) -> Result<WithdrawTestFlowRes> {
    // remember state
    let withdrawer_balance_before_withdrawing =
        algod.account_information(&creator.address()).await?.amount;

    let to_sign = withdraw(
        &algod,
        creator.address(),
        amount,
        project.votes_asset_id,
        &project.central_escrow,
        &project.votein_escrow,
        &project.vote_out_escrow,
    )
    .await?;

    // UI

    let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;
    let pay_vote_fee_tx_signed = creator.sign_transaction(&to_sign.pay_vote_fee_tx)?;

    let withdraw_tx_id = submit_withdraw(
        &algod,
        &WithdrawSigned {
            withdraw_tx: to_sign.withdraw_tx,
            pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
            consume_votes_tx: to_sign.consume_votes_tx,
            pay_vote_fee_tx: pay_vote_fee_tx_signed,
        },
    )
    .await?;

    wait_for_pending_transaction(&algod, &withdraw_tx_id).await?;

    Ok(WithdrawTestFlowRes {
        project: project.clone(),
        withdrawer_balance_before_withdrawing,
        withdrawal: amount,
    })
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawTestFlowRes {
    pub project: Project,
    pub withdrawer_balance_before_withdrawing: MicroAlgos,
    pub withdrawal: MicroAlgos,
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawTestPrecsRes {
    pub project: Project,
    pub central_escrow_balance_after_drain: MicroAlgos,
    pub drain_res: CustomerPaymentAndDrainFlowRes,
}
