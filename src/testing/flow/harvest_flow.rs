#[cfg(test)]
pub use test::{harvest_flow, harvest_precs, HarvestTestFlowRes, HarvestTestPrecsRes};

#[cfg(test)]
pub mod test {
    use crate::flows::create_project::{model::Project, share_amount::ShareAmount};
    use crate::funds::FundsAmount;
    use crate::state::account_state::funds_holdings;
    use crate::testing::flow::customer_payment_and_drain_flow::CustomerPaymentAndDrainFlowRes;
    use crate::testing::flow::invest_in_project_flow::invests_optins_flow;
    use crate::{
        flows::harvest::harvest::{harvest, submit_harvest, HarvestSigned},
        network_util::wait_for_pending_transaction,
        testing::flow::{
            create_project_flow::create_project_flow,
            customer_payment_and_drain_flow::customer_payment_and_drain_flow,
            invest_in_project_flow::invests_flow,
        },
        testing::network_test_util::TestDeps,
    };
    use algonaut::transaction::account::Account;
    use anyhow::Result;

    pub async fn harvest_precs(
        td: &TestDeps,
        share_amount: ShareAmount,
        payment_and_drain_amount: FundsAmount,
        drainer: &Account,
        harvester: &Account,
    ) -> Result<HarvestTestPrecsRes> {
        let algod = &td.algod;

        let project = create_project_flow(&td).await?;

        // investor buys shares: this can be called after draining as well (without affecting test results)
        // the only order required for this is draining->harvesting, obviously harvesting has to be executed after draining (if it's to harvest the drained funds)
        invests_optins_flow(algod, &harvester, &project.project).await?;
        let _ = invests_flow(
            &td,
            &harvester,
            share_amount,
            &project.project,
            &project.project_id,
        )
        .await?;

        // payment and draining
        let drain_res = customer_payment_and_drain_flow(
            td,
            &project.project,
            payment_and_drain_amount,
            &drainer,
        )
        .await?;

        let central_escrow_balance_after_drain = funds_holdings(
            algod,
            drain_res.project.central_escrow.address(),
            td.funds_asset_id,
        )
        .await?;

        // end precs

        Ok(HarvestTestPrecsRes {
            project: project.project,
            central_escrow_balance_after_drain,
            drain_res,
        })
    }

    pub async fn harvest_flow(
        td: &TestDeps,
        project: &Project,
        harvester: &Account,
        amount: FundsAmount,
    ) -> Result<HarvestTestFlowRes> {
        let algod = &td.algod;

        // remember state
        let harvester_balance_before_harvesting =
            funds_holdings(algod, &harvester.address(), td.funds_asset_id).await?;

        let to_sign = harvest(
            &algod,
            &harvester.address(),
            project.central_app_id,
            td.funds_asset_id,
            amount,
            &project.central_escrow,
        )
        .await?;

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

    // Any data we want to return from the flow to the tests
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HarvestTestFlowRes {
        pub project: Project,
        pub harvester_balance_before_harvesting: FundsAmount,
        pub harvest: FundsAmount,
    }

    // Any data we want to return from the flow to the tests
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HarvestTestPrecsRes {
        pub project: Project,
        pub central_escrow_balance_after_drain: FundsAmount,
        pub drain_res: CustomerPaymentAndDrainFlowRes,
    }
}
