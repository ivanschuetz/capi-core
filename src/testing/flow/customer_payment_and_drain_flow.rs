#[cfg(test)]
use crate::funds::FundsAmount;
#[cfg(test)]
use crate::funds::FundsAssetId;
#[cfg(test)]
use crate::{
    flows::create_project::model::Project,
    flows::create_project::storage::load_project::TxId,
    flows::drain::drain::{
        drain_customer_escrow, submit_drain_customer_escrow, DrainCustomerEscrowSigned,
    },
    flows::pay_project::pay_project::{pay_project, submit_pay_project, PayProjectSigned},
    network_util::wait_for_pending_transaction,
    state::account_state::funds_holdings,
};
#[cfg(test)]
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{account::Account, Transaction},
};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn customer_payment_and_drain_flow(
    algod: &Algod,
    drainer: &Account,
    customer: &Account,
    funds_asset_id: FundsAssetId,
    customer_payment_amount: FundsAmount,
    project: &Project,
) -> Result<CustomerPaymentAndDrainFlowRes> {
    // double check precondition: customer escrow has no funds
    let customer_escrow_holdings =
        funds_holdings(algod, project.customer_escrow.address(), funds_asset_id).await?;
    assert_eq!(FundsAmount(0), customer_escrow_holdings);

    // Customer sends a payment
    let customer_payment_tx_id = send_payment_to_customer_escrow(
        &algod,
        &customer,
        project.customer_escrow.address(),
        funds_asset_id,
        customer_payment_amount,
    )
    .await?;
    wait_for_pending_transaction(&algod, &customer_payment_tx_id).await?;

    drain_flow(algod, drainer, project).await
}

#[cfg(test)]
pub async fn drain_flow(
    algod: &Algod,
    drainer: &Account,
    project: &Project,
) -> Result<CustomerPaymentAndDrainFlowRes> {
    let initial_drainer_balance = algod.account_information(&drainer.address()).await?.amount;

    let drain_to_sign = drain_customer_escrow(
        &algod,
        &drainer.address(),
        project.central_app_id,
        project.funds_asset_id,
        &project.customer_escrow,
        &project.central_escrow,
    )
    .await?;

    let pay_fee_tx_signed = drainer.sign_transaction(&drain_to_sign.pay_fee_tx)?;
    let app_call_tx_signed = drainer.sign_transaction(&drain_to_sign.app_call_tx)?;

    let drain_tx_id = submit_drain_customer_escrow(
        &algod,
        &DrainCustomerEscrowSigned {
            drain_tx: drain_to_sign.drain_tx,
            pay_fee_tx: pay_fee_tx_signed,
            app_call_tx_signed,
        },
    )
    .await?;

    wait_for_pending_transaction(&algod, &drain_tx_id).await?;

    Ok(CustomerPaymentAndDrainFlowRes {
        project: project.to_owned(),
        initial_drainer_balance,
        pay_fee_tx: drain_to_sign.pay_fee_tx,
        app_call_tx: drain_to_sign.app_call_tx,
        drained_amount: drain_to_sign.amount_to_drain, // txs were submitted successfully: already drained
    })
}

// Any data we want to return from the flow to the tests
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerPaymentAndDrainFlowRes {
    pub project: Project,
    pub initial_drainer_balance: MicroAlgos,
    pub pay_fee_tx: Transaction,
    pub app_call_tx: Transaction,
    pub drained_amount: FundsAmount,
}

// Simulate a payment to the "external" project address
#[cfg(test)]
async fn send_payment_to_customer_escrow(
    algod: &Algod,
    customer: &Account,
    customer_escrow: &Address,
    funds_asset_id: FundsAssetId,
    amount: FundsAmount,
) -> Result<TxId> {
    let tx = pay_project(
        algod,
        &customer.address(),
        customer_escrow,
        funds_asset_id,
        amount,
    )
    .await?
    .tx;
    let signed_tx = customer.sign_transaction(&tx)?;
    let tx_id = submit_pay_project(algod, PayProjectSigned { tx: signed_tx }).await?;
    log::debug!("Customer payment tx id: {:?}", tx_id);
    Ok(tx_id)
}
