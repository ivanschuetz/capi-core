#[cfg(test)]
use super::customer_payment_and_drain_flow::CustomerPaymentAndDrainFlowRes;
#[cfg(test)]
use super::invest_in_project_flow::invests_optins_flow;
#[cfg(test)]
use crate::flows::create_project::{
    create_project_specs::CreateProjectSpecs, model::Project, share_amount::ShareAmount,
};
#[cfg(test)]
use crate::funds::{FundsAmount, FundsAssetId};
#[cfg(test)]
use crate::state::account_state::funds_holdings;
#[cfg(test)]
use crate::{
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    flows::harvest::harvest::{harvest, submit_harvest, HarvestSigned},
    network_util::wait_for_pending_transaction,
    testing::flow::{
        create_project_flow::create_project_flow,
        customer_payment_and_drain_flow::customer_payment_and_drain_flow,
        invest_in_project_flow::invests_flow,
    },
};
#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn harvest_precs(
    algod: &Algod,
    creator: &Account,
    specs: &CreateProjectSpecs,
    funds_asset_id: FundsAssetId,
    harvester: &Account,
    drainer: &Account,
    customer: &Account,
    share_amount: ShareAmount,
    payment_and_drain_amount: FundsAmount,
    precision: u64,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<HarvestTestPrecsRes> {
    let project = create_project_flow(
        &algod,
        &creator,
        &specs,
        funds_asset_id,
        precision,
        capi_deps,
    )
    .await?;

    // investor buys shares: this can be called after draining as well (without affecting test results)
    // the only order required for this is draining->harvesting, obviously harvesting has to be executed after draining (if it's to harvest the drained funds)
    invests_optins_flow(&algod, &harvester, &project.project).await?;
    let _ = invests_flow(
        &algod,
        &harvester,
        share_amount,
        funds_asset_id,
        &project.project,
        &project.project_id,
    )
    .await?;

    // payment and draining
    let drain_res = customer_payment_and_drain_flow(
        &algod,
        &drainer,
        &customer,
        funds_asset_id,
        payment_and_drain_amount,
        &project.project,
        capi_deps,
    )
    .await?;

    let central_escrow_balance_after_drain = funds_holdings(
        algod,
        drain_res.project.central_escrow.address(),
        funds_asset_id,
    )
    .await?;

    // end precs

    Ok(HarvestTestPrecsRes {
        project: project.project,
        central_escrow_balance_after_drain,
        drain_res,
    })
}

#[cfg(test)]
pub async fn harvest_flow(
    algod: &Algod,
    project: &Project,
    harvester: &Account,
    funds_asset_id: FundsAssetId,
    amount: FundsAmount,
) -> Result<HarvestTestFlowRes> {
    // remember state
    let harvester_balance_before_harvesting =
        funds_holdings(algod, &harvester.address(), funds_asset_id).await?;

    let to_sign = harvest(
        &algod,
        &harvester.address(),
        project.central_app_id,
        funds_asset_id,
        amount,
        &project.central_escrow,
    )
    .await?;

    // UI

    let app_call_tx_signed = harvester.sign_transaction(&to_sign.app_call_tx)?;

    let harvest_tx_id = submit_harvest(
        &algod,
        &HarvestSigned {
            app_call_tx_signed,
            harvest_tx: to_sign.harvest_tx,
        },
    )
    .await?;

    wait_for_pending_transaction(&algod, &harvest_tx_id).await?;

    Ok(HarvestTestFlowRes {
        project: project.clone(),
        harvester_balance_before_harvesting,
        harvest: amount.to_owned(),
        // drain_res: precs.drain_res.clone(),
    })
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestTestFlowRes {
    pub project: Project,
    pub harvester_balance_before_harvesting: FundsAmount,
    pub harvest: FundsAmount,
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarvestTestPrecsRes {
    pub project: Project,
    pub central_escrow_balance_after_drain: FundsAmount,
    pub drain_res: CustomerPaymentAndDrainFlowRes,
}
