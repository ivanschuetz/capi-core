#[cfg(test)]
use super::customer_payment_and_drain_flow::CustomerPaymentAndDrainFlowRes;
#[cfg(test)]
use crate::flows::create_project::model::{CreateProjectSpecs, Project};
#[cfg(test)]
use crate::{
    flows::harvest::logic::{harvest, submit_harvest, HarvestSigned},
    network_util::wait_for_pending_transaction,
    testing::flow::{
        create_project::create_project_flow,
        customer_payment_and_drain_flow::customer_payment_and_drain_flow,
        invest_in_project::invests_flow,
    },
};
#[cfg(test)]
use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn harvest_precs(
    algod: &Algod,
    creator: &Account,
    specs: &CreateProjectSpecs,
    harvester: &Account,
    drainer: &Account,
    customer: &Account,
    buy_asset_amount: u64, // UI
    withdrawal_slots: u64,
    central_funds: MicroAlgos,
) -> Result<HarvestTestPrecsRes> {
    use super::invest_in_project::invests_optins_flow;

    let project = create_project_flow(&algod, &creator, &specs, withdrawal_slots).await?;

    // investor buys shares: this can be called after draining as well (without affecting test results)
    // the only order required for this is draining->harvesting, obviously harvesting has to be executed after draining (if it's to harvest the drained funds)
    invests_optins_flow(&algod, &harvester, &project).await?;
    let _ = invests_flow(&algod, &harvester, buy_asset_amount, &project).await?;

    // payment and draining
    let drain_res =
        customer_payment_and_drain_flow(&algod, &drainer, &customer, central_funds, &project)
            .await?;
    let central_escrow_balance_after_drain = algod
        .account_information(&drain_res.project.central_escrow.address)
        .await?
        .amount;

    // end precs

    Ok(HarvestTestPrecsRes {
        project,
        central_escrow_balance_after_drain,
        drain_res,
    })
}

#[cfg(test)]
pub async fn harvest_flow(
    algod: &Algod,
    project: &Project,
    harvester: &Account,
    amount: MicroAlgos,
) -> Result<HarvestTestFlowRes> {
    // remember state
    let harvester_balance_before_harvesting = algod
        .account_information(&harvester.address())
        .await?
        .amount;

    let to_sign = harvest(
        &algod,
        &harvester.address(),
        project.central_app_id,
        amount,
        &project.central_escrow,
    )
    .await?;

    // UI

    let app_call_tx_signed = harvester.sign_transaction(&to_sign.app_call_tx)?;
    let pay_fee_tx_signed = harvester.sign_transaction(&to_sign.pay_fee_tx)?;

    let harvest_tx_id = submit_harvest(
        &algod,
        &HarvestSigned {
            app_call_tx_signed,
            harvest_tx: to_sign.harvest_tx,
            pay_fee_tx: pay_fee_tx_signed,
        },
    )
    .await?;

    wait_for_pending_transaction(&algod, &harvest_tx_id).await?;

    Ok(HarvestTestFlowRes {
        project: project.clone(),
        harvester_balance_before_harvesting,
        harvest: amount,
        // drain_res: precs.drain_res.clone(),
    })
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestTestFlowRes {
    pub project: Project,
    pub harvester_balance_before_harvesting: MicroAlgos,
    pub harvest: MicroAlgos,
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestTestPrecsRes {
    pub project: Project,
    pub central_escrow_balance_after_drain: MicroAlgos,
    pub drain_res: CustomerPaymentAndDrainFlowRes,
}
