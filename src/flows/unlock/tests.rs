#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        flows::create_project::share_amount::ShareAmount,
        funds::FundsAmount,
        network_util::wait_for_pending_transaction,
        state::{
            account_state::find_asset_holding_or_err,
            central_app_state::central_investor_state_from_acc,
        },
        testing::{
            flow::{
                create_project_flow::create_project_flow,
                invest_in_project_flow::{invests_flow, invests_optins_flow},
                unlock_flow::unlock_flow,
            },
            network_test_util::test_dao_init,
        },
    };

    #[test]
    #[serial]
    async fn test_unlock() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let buy_share_amount = ShareAmount::new(10);

        // precs

        let project = create_project_flow(td).await?;

        invests_optins_flow(&algod, &investor, &project.project).await?;
        let _ = invests_flow(
            td,
            investor,
            buy_share_amount,
            &project.project,
            &project.project_id,
        )
        .await?;
        // TODO double check tests for state (at least important) tested (e.g. investor has shares, locking doesn't etc.)

        // double check investor's assets

        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset =
            find_asset_holding_or_err(&investor_assets, project.project.shares_asset_id)?;
        // doesn't have shares (they're sent directly to locking escrow)
        assert_eq!(0, shares_asset.amount);

        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.project.central_app_id)?;
        // double check investor's local state
        // shares set to bought asset amount
        assert_eq!(buy_share_amount, investor_state.shares);
        //  harvested total is 0 (hasn't harvested yet)
        assert_eq!(FundsAmount::new(0), investor_state.harvested);

        // double check locking escrow's assets
        let locking_escrow_infos = algod
            .account_information(project.project.locking_escrow.address())
            .await?;
        let locking_escrow_assets = locking_escrow_infos.assets;

        assert_eq!(1, locking_escrow_assets.len()); // opted in to shares
        assert_eq!(buy_share_amount.0, locking_escrow_assets[0].amount);

        // flow

        // in the real application, unlock_share_amount is retrieved from indexer
        let unlock_share_amount = buy_share_amount;

        let unlock_tx_id =
            unlock_flow(algod, &project.project, investor, unlock_share_amount).await?;
        let _ = wait_for_pending_transaction(algod, &unlock_tx_id).await?;

        // shares not anymore in locking escrow
        let locking_escrow_infos = algod
            .account_information(project.project.locking_escrow.address())
            .await?;
        let locking_escrow_assets = locking_escrow_infos.assets;
        assert_eq!(1, locking_escrow_assets.len()); // still opted in to shares
        assert_eq!(0, locking_escrow_assets[0].amount); // lost shares

        // investor got shares
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset =
            find_asset_holding_or_err(&investor_assets, project.project.shares_asset_id)?;
        // got the shares
        assert_eq!(buy_share_amount.0, shares_asset.amount);

        // investor local state cleared (opted out)
        assert_eq!(0, investor_infos.apps_local_state.len());

        Ok(())
    }

    // TODO think how to implement partial unlocking: it should be common that investors want to sell only a part of their shares
    // currently we require opt-out to prevent double harvest, REVIEW
    #[test]
    #[serial]
    async fn test_partial_unlock_not_allowed() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        let partial_amount = ShareAmount::new(2);
        let buy_asset_amount = ShareAmount::new(partial_amount.val() + 8);

        // precs

        let project = create_project_flow(&td).await?;

        invests_optins_flow(algod, investor, &project.project).await?;
        let _ = invests_flow(
            td,
            investor,
            buy_asset_amount,
            &project.project,
            &project.project_id,
        )
        .await?;

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset =
            find_asset_holding_or_err(&investor_assets, project.project.shares_asset_id)?;
        // doesn't have shares (they're sent directly to locking escrow)
        assert_eq!(0, shares_asset.amount);

        // double check investor's local state
        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.project.central_app_id)?;
        // shares set to bought asset amount
        assert_eq!(buy_asset_amount, investor_state.shares);
        // harvested total is 0 (hasn't harvested yet)
        assert_eq!(FundsAmount::new(0), investor_state.harvested);

        // double check locking escrow's assets
        let locking_escrow_infos = algod
            .account_information(project.project.locking_escrow.address())
            .await?;
        let locking_escrow_assets = locking_escrow_infos.assets;
        assert_eq!(1, locking_escrow_assets.len()); // opted in to shares
        assert_eq!(buy_asset_amount.0, locking_escrow_assets[0].amount);

        // remember state
        let investor_balance_before_unlocking = investor_infos.amount;

        // flow

        let unlock_share_amount = partial_amount;

        let unlock_result =
            unlock_flow(algod, &project.project, investor, unlock_share_amount).await;

        assert!(unlock_result.is_err());

        // shares still in locking escrow
        let locking_escrow_infos = algod
            .account_information(project.project.locking_escrow.address())
            .await?;
        let locking_escrow_assets = locking_escrow_infos.assets;
        assert_eq!(1, locking_escrow_assets.len()); // still opted in to shares
        assert_eq!(buy_asset_amount.0, locking_escrow_assets[0].amount); // lost shares

        // investor didn't get anything

        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        // funds asset + shares asset
        assert_eq!(2, investor_assets.len());
        let shares_asset =
            find_asset_holding_or_err(&investor_assets, project.project.shares_asset_id)?;
        // no shares
        assert_eq!(0, shares_asset.amount);

        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.project.central_app_id)?;
        // investor local state not changed
        // shares set to bought asset amount
        assert_eq!(buy_asset_amount, investor_state.shares);
        // harvested total is 0 (hasn't harvested yet)
        assert_eq!(FundsAmount::new(0), investor_state.harvested);

        // investor didn't pay fees (unlock txs failed)
        assert_eq!(investor_balance_before_unlocking, investor_infos.amount);

        Ok(())
    }
}
