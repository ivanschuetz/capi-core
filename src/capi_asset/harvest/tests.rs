#[cfg(test)]
mod tests {
    use crate::{
        capi_asset::{
            capi_app_state::{capi_app_global_state, capi_app_investor_state},
            capi_asset_id::CapiAssetAmount,
        },
        funds::FundsAmount,
        state::account_state::funds_holdings,
        testing::{
            flow::harvest_capi_flow::{harvest_capi_flow, harvest_capi_precs},
            network_test_util::test_dao_init,
        },
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_harvest() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let investor_capi_amount = CapiAssetAmount::new(100_000); // 0.0001 -> 0.01 %

        let initial_capi_funds_amount = FundsAmount::new(200_000);

        let harvest_amount = FundsAmount::new(2); // random amount < entitled harvest

        // preconditions

        harvest_capi_precs(
            td,
            &td.capi_owner,
            investor,
            investor_capi_amount,
            initial_capi_funds_amount,
        )
        .await?;

        // flow

        let investor_funds_before_harvesting =
            funds_holdings(algod, &investor.address(), td.funds_asset_id).await?;

        let capi_app_total_received_before_harvesting =
            capi_app_global_state(algod, td.capi_app_id).await?.received;

        harvest_capi_flow(
            algod,
            investor,
            harvest_amount,
            td.funds_asset_id,
            td.capi_app_id,
            &td.capi_escrow,
        )
        .await?;

        // test

        // Investor received the harvested funds
        let harvest_funds_amount =
            funds_holdings(algod, &investor.address(), td.funds_asset_id).await?;
        assert_eq!(
            investor_funds_before_harvesting.val() + harvest_amount.val(),
            harvest_funds_amount.val()
        );

        // Capi lost the harvested funds
        let capi_escrow_funds_amount =
            funds_holdings(algod, &td.capi_escrow.address(), td.funds_asset_id).await?;
        assert_eq!(
            initial_capi_funds_amount.val() - harvest_amount.val(),
            capi_escrow_funds_amount.val()
        );

        // Capi app global state: test that the total received global variable didn't change (unaffected by harvesting)
        let capi_app_global_state = capi_app_global_state(&algod, td.capi_app_id).await?;
        assert_eq!(
            capi_app_total_received_before_harvesting,
            capi_app_global_state.received
        );

        // Investor local state: test that it was incremented by amount harvested
        let investor_local_state =
            capi_app_investor_state(algod, &investor.address(), td.capi_app_id).await?;
        // harvested local state is just what they just harvested (there wasn't anything on the escrow when the investor invested)
        assert_eq!(harvest_amount.0, investor_local_state.harvested.0);
        // sanity check: the shares local state is set to the locked shares
        assert_eq!(investor_capi_amount.0, investor_local_state.shares.0);

        Ok(())
    }

    // TODO rest of tests from DAO harvest
}
