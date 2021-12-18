#[cfg(test)]
use crate::{
    flows::create_project::model::Project,
    flows::drain::drain::{
        drain_customer_escrow, submit_drain_customer_escrow, DrainCustomerEscrowSigned,
    },
    network_util::wait_for_pending_transaction,
};
#[cfg(test)]
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{account::Account, Pay, Transaction, TxnBuilder},
};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn customer_payment_and_drain_flow(
    algod: &Algod,
    drainer: &Account,
    customer: &Account,
    customer_payment_amount: MicroAlgos,
    project: &Project,
) -> Result<CustomerPaymentAndDrainFlowRes> {
    // Customer sends a payment
    let customer_payment_tx_id = send_payment_to_customer_escrow(
        &algod,
        &customer,
        &project.customer_escrow.address,
        customer_payment_amount,
    )
    .await?;
    wait_for_pending_transaction(&algod, &customer_payment_tx_id).await?;

    let customer_escrow_balance = algod
        .account_information(&project.customer_escrow.address)
        .await?
        .amount;
    // let central_escrow_balance = algod
    //     .account_information(&project.central_escrow.address)
    //     .await?
    //     .amount;
    // remember initial drainer balance
    let initial_drainer_balance = algod.account_information(&drainer.address()).await?.amount;
    // TODO check whether these double checks are really necessary and move to tests if needed, flows should not make assumptions about context
    // for example, when using this in staking tests it fails, because we call it multiple times (with different contexts)
    // // double check that the payment arrived on the customer escrow
    // // normally should be part of test but too complicated to split here - it's just a double check anyway
    // assert_eq!(
    //     MIN_BALANCE + customer_payment_amount + FIXED_FEE,
    //     customer_escrow_balance
    // );
    // // double check that there's nothing on central yet
    // // normally should be part of test but too complicated to split here - it's just a double check anyway
    // // Note + FIXED_FEE, we add FIXED_FEE to min balance when creating project (central_escrow.rs)
    // // to not fail when withdrawing everything
    // // TODO clarify: how are the groups evaluated, better way.
    // assert_eq!(MIN_BALANCE + FIXED_FEE, central_escrow_balance);

    // Someone triggers harvest from customer escrow
    // UI
    let drain_to_sign = drain_customer_escrow(
        &algod,
        &drainer.address(),
        project.central_app_id,
        &project.customer_escrow,
        &project.central_escrow,
    )
    .await?;

    let pay_fee_tx_signed = drainer.sign_transaction(&drain_to_sign.pay_fee_tx)?;
    let app_call_tx_signed = drainer.sign_transaction(&drain_to_sign.app_call_tx)?;

    println!(
        "customer_escrow_balance before drain: {:?}",
        customer_escrow_balance
    );

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
        drained_amount: drain_to_sign.amount_to_drain, // the txs were successful here: already drained
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
    pub drained_amount: MicroAlgos,
}

// Simulate a payment to the "external" project address
#[cfg(test)]
async fn send_payment_to_customer_escrow(
    algod: &Algod,
    customer: &Account,
    escrow_address: &Address,
    amount: MicroAlgos,
) -> Result<String> {
    let params = algod.suggested_transaction_params().await?;

    let tx = &mut TxnBuilder::with(
        params.clone(),
        Pay::new(customer.address(), *escrow_address, amount).build(),
    )
    .build();

    let signed_tx = customer.sign_transaction(tx)?;
    let res = algod.broadcast_signed_transaction(&signed_tx).await?;

    println!("Customer payment tx id: {:?}", res.tx_id);

    Ok(res.tx_id)
}
