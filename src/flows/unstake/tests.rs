#[cfg(test)]
mod tests {
    use algonaut::core::MicroAlgos;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::unstake::unstake::FIXED_FEE,
        network_util::wait_for_pending_transaction,
        state::central_app_state::central_investor_state_from_acc,
        testing::{
            flow::{
                create_project_flow::create_project_flow,
                invest_in_project_flow::{invests_flow, invests_optins_flow},
                unstake_flow::unstake_flow,
            },
            network_test_util::test_init,
            test_data::{creator, investor1, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
    };

    #[test]
    #[serial]
    async fn test_unstake() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();

        // UI

        let buy_asset_amount = 10;

        // precs

        let project =
            create_project_flow(&algod, &creator, &project_specs(), TESTS_DEFAULT_PRECISION)
                .await?;

        invests_optins_flow(&algod, &investor, &project).await?;
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;
        // TODO double check tests for state (at least important) tested (e.g. investor has shares, staking doesn't etc.)

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // opted in to shares
        assert_eq!(0, investor_assets[0].amount); // doesn't have shares (they're sent directly to staking escrow)

        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.central_app_id)?;
        // double check investor's local state
        // shares set to bought asset amount
        assert_eq!(buy_asset_amount, investor_state.shares);
        //  harvested total is 0 (hasn't harvested yet)
        assert_eq!(MicroAlgos(0), investor_state.harvested);

        // double check staking escrow's assets
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len()); // opted in to shares
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);

        // remember state
        let investor_balance_before_unstaking = investor_infos.amount;

        // flow

        // in the real application, unstake_share_amount is retrieved from indexer
        let unstake_share_amount = buy_asset_amount;

        let unstake_tx_id = unstake_flow(&algod, &project, &investor, unstake_share_amount).await?;
        println!("?? unstake tx id: {:?}", unstake_tx_id);
        let _ = wait_for_pending_transaction(&algod, &unstake_tx_id).await?;

        // shares not anymore in staking escrow
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len()); // still opted in to shares
        assert_eq!(0, staking_escrow_assets[0].amount); // lost shares

        // investor got shares
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len());
        assert_eq!(buy_asset_amount, investor_assets[0].amount); // got the shares

        // investor local state cleared (opted out)
        assert_eq!(0, investor_infos.apps_local_state.len());

        // investor paid the fees (app call + xfer + xfer fee)
        assert_eq!(
            investor_balance_before_unstaking - FIXED_FEE * 3,
            investor_infos.amount
        );

        Ok(())
    }

    // TODO think how to implement partial unstaking: it should be common that investors want to sell only a part of their shares
    // currently we require opt-out to prevent double harvest, REVIEW
    #[test]
    #[serial]
    async fn test_partial_unstake_not_allowed() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();

        // UI

        let partial_amount = 2;
        let buy_asset_amount = partial_amount + 8;

        // precs

        let project =
            create_project_flow(&algod, &creator, &project_specs(), TESTS_DEFAULT_PRECISION)
                .await?;

        invests_optins_flow(&algod, &investor, &project).await?;
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // opted in to shares
        assert_eq!(0, investor_assets[0].amount); // doesn't have shares (they're sent directly to staking escrow)

        // double check investor's local state
        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.central_app_id)?;
        // shares set to bought asset amount
        assert_eq!(buy_asset_amount, investor_state.shares);
        // harvested total is 0 (hasn't harvested yet)
        assert_eq!(MicroAlgos(0), investor_state.harvested);

        // double check staking escrow's assets
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len()); // opted in to shares
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);

        // remember state
        let investor_balance_before_unstaking = investor_infos.amount;

        // flow

        let unstake_share_amount = partial_amount;

        let unstake_result = unstake_flow(&algod, &project, &investor, unstake_share_amount).await;

        assert!(unstake_result.is_err());

        // shares still in staking escrow
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len()); // still opted in to shares
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount); // lost shares

        // investor didn't get anything
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        assert_eq!(1, investor_assets.len());
        assert_eq!(0, investor_assets[0].amount); // no shares

        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.central_app_id)?;
        // investor local state not changed
        // shares set to bought asset amount
        assert_eq!(buy_asset_amount, investor_state.shares);
        // harvested total is 0 (hasn't harvested yet)
        assert_eq!(MicroAlgos(0), investor_state.harvested);

        // investor didn't pay fees (unstake txs failed)
        assert_eq!(investor_balance_before_unstaking, investor_infos.amount);

        Ok(())
    }
}
