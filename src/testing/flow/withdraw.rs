#[cfg(test)]
use super::customer_payment_and_drain_flow::CustomerPaymentAndDrainFlowRes;
#[cfg(test)]
use super::invest_in_project::invests_optins_flow;
#[cfg(test)]
use crate::flows::{
    create_project::model::Project,
    withdraw::logic::{submit_withdraw, withdraw, WithdrawSigned},
};
#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use crate::testing::flow::{
    customer_payment_and_drain_flow::customer_payment_and_drain_flow,
    init_withdrawal::init_withdrawal_flow, invest_in_project::invests_flow, vote::vote_flow,
};
#[cfg(test)]
use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

/// project creation,
/// customer payment + draining to central, to have something to withdraw.
/// withdrawal request initialization
/// voting to meet request threshold
#[cfg(test)]
pub async fn withdraw_precs(
    algod: &Algod,
    creator: &Account,
    drainer: &Account,
    customer: &Account,
    voter: &Account,
    project: &Project,
    pay_and_drain_amount: MicroAlgos,
    amount_to_withdraw: MicroAlgos,
    slot_id: u64,
) -> Result<WithdrawTestPrecsRes> {
    // customer payment and draining, to have some funds to withdraw
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

    // Investor buys shares with count == vote threshold count
    let investor_shares_count = project.specs.vote_threshold_units();
    invests_optins_flow(&algod, &voter, &project).await?;
    invests_flow(algod, voter, investor_shares_count, &project).await?;

    // Init a request
    let init_withdrawal_tx_id =
        init_withdrawal_flow(&algod, &creator, amount_to_withdraw, slot_id).await?;
    wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

    // Vote with enough shares so withdrawal will pass
    let vote_tx_id = vote_flow(algod, voter, project, slot_id, investor_shares_count).await?;
    wait_for_pending_transaction(&algod, &vote_tx_id).await?;

    Ok(WithdrawTestPrecsRes {
        central_escrow_balance_after_drain,
        drain_res,
        owned_shares: investor_shares_count,
    })
}

#[cfg(test)]
pub async fn withdraw_flow(
    algod: &Algod,
    project: &Project,
    creator: &Account,
    amount: MicroAlgos,
    slot_id: u64,
) -> Result<WithdrawTestFlowRes> {
    // remember state
    let withdrawer_balance_before_withdrawing =
        algod.account_information(&creator.address()).await?.amount;

    let to_sign = withdraw(
        &algod,
        creator.address(),
        amount,
        &project.central_escrow,
        slot_id,
    )
    .await?;

    // UI
    let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;
    let check_enough_votes_tx_signed = creator.sign_transaction(&to_sign.check_enough_votes_tx)?;

    let withdraw_tx_id = submit_withdraw(
        &algod,
        &WithdrawSigned {
            withdraw_tx: to_sign.withdraw_tx,
            pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
            check_enough_votes_tx: check_enough_votes_tx_signed,
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
    pub central_escrow_balance_after_drain: MicroAlgos,
    pub drain_res: CustomerPaymentAndDrainFlowRes,
    // the share count bought to perform the vote (and still owned)
    pub owned_shares: u64,
}
