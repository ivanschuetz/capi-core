#[cfg(test)]
use super::customer_payment_and_drain_flow::CustomerPaymentAndDrainFlowRes;
#[cfg(test)]
use crate::flows::{
    create_project::model::Project,
    withdraw::withdraw::{submit_withdraw, withdraw, WithdrawSigned, WithdrawalInputs},
};
#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use crate::testing::flow::customer_payment_and_drain_flow::customer_payment_and_drain_flow;
#[cfg(test)]
use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

/// project creation,
/// customer payment + draining to central, to have something to withdraw.
#[cfg(test)]
pub async fn withdraw_precs(
    algod: &Algod,
    drainer: &Account,
    customer: &Account,
    project: &Project,
    pay_and_drain_amount: MicroAlgos,
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
        .account_information(drain_res.project.central_escrow.address())
        .await?
        .amount;

    Ok(WithdrawTestPrecsRes {
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
        &WithdrawalInputs {
            amount,
            description: "Withdrawing from tests".to_owned(),
        },
        &project.central_escrow,
    )
    .await?;

    // UI
    let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;

    let withdraw_tx_id = submit_withdraw(
        &algod,
        &WithdrawSigned {
            withdraw_tx: to_sign.withdraw_tx,
            pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
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
}
