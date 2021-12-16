#[cfg(test)]
mod tests {
    use algonaut::{
        core::MicroAlgos,
        transaction::{AcceptAsset, TransferAsset, TxnBuilder},
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::{
            harvest::logic::calculate_entitled_harvest,
            invest::app_optins::{
                invest_or_staking_app_optins_txs, submit_invest_or_staking_app_optins,
            },
            stake::logic::FIXED_FEE,
        },
        network_util::wait_for_pending_transaction,
        state::central_app_state::central_investor_state_from_acc,
        testing::{
            flow::{
                create_project::create_project_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
                harvest::harvest_flow,
                invest_in_project::{invests_flow, invests_optins_flow},
                stake::stake_flow,
                unstake::unstake_flow,
            },
            network_test_util::reset_network,
            test_data::{self, creator, customer, investor1, investor2, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
    };

    #[test]
    #[serial]
    async fn test_stake() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor1 = investor1();
        let investor2 = investor2();
        // repurposing creator as drainer here, as there are only 2 investor test accounts and we prefer them in a clean state for these tests
        let drainer = test_data::creator();
        let customer = customer();

        // UI

        let buy_asset_amount = 10;

        // precs

        let project =
            create_project_flow(&algod, &creator, &project_specs(), TESTS_DEFAULT_PRECISION)
                .await?;

        invests_optins_flow(&algod, &investor1, &project).await?;
        let _ = invests_flow(&algod, &investor1, buy_asset_amount, &project).await?;

        // drain (to generate dividend). note that investor doesn't reclaim it (doesn't seem relevant for this test)
        // (the draining itself may also not be relevant, just for a more realistic pre-trade scenario)
        let customer_payment_amount = MicroAlgos(10 * 1_000_000);
        let _ = customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            customer_payment_amount,
            &project,
        )
        .await?;

        // investor1 unstakes
        let traded_shares = buy_asset_amount;
        let unstake_tx_id = unstake_flow(&algod, &project, &investor1, traded_shares).await?;
        let _ = wait_for_pending_transaction(&algod, &unstake_tx_id).await?;

        // investor2 gets shares from investor1 externally
        // normally this will be a swap in a dex. could also be a gift or some other service

        // investor2 opts in to the asset (this is done in the external service, e.g. dex)
        let params = algod.suggested_transaction_params().await?;
        let shares_optin_tx = &mut TxnBuilder::with(
            params.clone(),
            AcceptAsset::new(investor2.address(), project.shares_asset_id).build(),
        )
        .build();
        let signed_shares_optin_tx = investor2.sign_transaction(shares_optin_tx)?;
        let res = algod
            .broadcast_signed_transaction(&signed_shares_optin_tx)
            .await?;
        let _ = wait_for_pending_transaction(&algod, &res.tx_id);

        // investor1 sends shares to investor2 (e.g. as part of atomic swap in a dex)
        let trade_tx = &mut TxnBuilder::with(
            params.clone(),
            TransferAsset::new(
                investor1.address(),
                project.shares_asset_id,
                traded_shares,
                investor2.address(),
            )
            .build(),
        )
        .build();
        let signed_trade_tx = investor1.sign_transaction(trade_tx)?;
        let res = algod.broadcast_signed_transaction(&signed_trade_tx).await?;
        let _ = wait_for_pending_transaction(&algod, &res.tx_id);

        // investor2 opts in to our app. this will be on our website.
        // TODO confirm: can't we opt in in the same group (accessing local state during opt in fails)?,
        // is there a way to avoid the investor confirming txs 2 times here?

        let app_optins_txs =
            invest_or_staking_app_optins_txs(&algod, &project, &investor2.address()).await?;
        // UI
        let mut app_optins_signed_txs = vec![];
        for optin_tx in app_optins_txs {
            app_optins_signed_txs.push(investor2.sign_transaction(&optin_tx)?);
        }
        let app_optins_tx_id =
            submit_invest_or_staking_app_optins(&algod, app_optins_signed_txs).await?;
        let _ = wait_for_pending_transaction(&algod, &app_optins_tx_id);

        // flow

        // investor2 stakes the acquired shares
        stake_flow(&algod, &project, &investor2, traded_shares).await?;

        // tests

        // investor2 lost staked assets
        let investor2_infos = algod.account_information(&investor2.address()).await?;
        let investor2_assets = &investor2_infos.assets;
        assert_eq!(1, investor2_assets.len()); // opted in to shares
        assert_eq!(0, investor2_assets[0].amount);

        // already harvested local state initialized to entitled algos

        // the amount drained to the central (all income so far)
        let central_total_received = customer_payment_amount;
        let investor2_entitled_amount = calculate_entitled_harvest(
            central_total_received,
            project.specs.shares.count,
            traded_shares,
            TESTS_DEFAULT_PRECISION,
            project.specs.investors_share,
        );

        let investor_state =
            central_investor_state_from_acc(&investor2_infos, project.central_app_id)?;
        // shares local state initialized to shares
        assert_eq!(traded_shares, investor_state.shares);
        // harvested total is initialized to entitled amount
        assert_eq!(investor2_entitled_amount, investor_state.harvested);

        // renaming for clarity
        let total_withdrawn_after_staking_setup_call = investor2_entitled_amount;

        // staking escrow got assets
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len()); // opted in to shares
        assert_eq!(traded_shares, staking_escrow_assets[0].amount);

        // investor2 harvests: doesn't get anything, because there has not been new income (customer payments) since they bought the shares
        // the harvest amount is the smallest number possible, to show that we can't retrieve anything
        let harvest_flow_res = harvest_flow(&algod, &project, &investor2, MicroAlgos(1)).await;
        println!("Expected error harvesting: {:?}", harvest_flow_res);
        // If there's nothing to harvest, the smart contract fails (transfer amount > allowed)
        assert!(harvest_flow_res.is_err());

        // drain again to generate dividend and be able to harvest
        let customer_payment_amount_2 = MicroAlgos(10 * 1_000_000);
        let _ = customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            customer_payment_amount_2,
            &project,
        )
        .await?;

        // harvest again: this time there's something to harvest and we expect success

        // remember state
        let investor2_amount_before_harvest = algod
            .account_information(&investor2.address())
            .await?
            .amount;

        // we'll harvest the max possible amount
        let investor2_entitled_amount = calculate_entitled_harvest(
            customer_payment_amount_2,
            project.specs.shares.count,
            traded_shares,
            TESTS_DEFAULT_PRECISION,
            project.specs.investors_share,
        );
        println!(
            "Harvesting max possible amount (expected to succeed): {:?}",
            investor2_entitled_amount
        );
        let _ = harvest_flow(&algod, &project, &investor2, investor2_entitled_amount).await?;
        // just a rename to help with clarity a bit
        let expected_harvested_amount = investor2_entitled_amount;
        let investor2_infos = algod.account_information(&investor2.address()).await?;
        // the balance is increased with the harvest - fees for the harvesting txs (app call + pay for harvest tx fee + fee for this tx)
        assert_eq!(
            investor2_amount_before_harvest + expected_harvested_amount - FIXED_FEE * 3,
            investor2_infos.amount
        );

        // investor's harvested local state was updated:
        let investor_state =
            central_investor_state_from_acc(&investor2_infos, project.central_app_id)?;
        // the shares haven't changed
        assert_eq!(traded_shares, investor_state.shares);
        // the harvested total was updated:
        // initial (total_withdrawn_after_staking_setup_call: entitled amount when staking the shares) + just harvested
        assert_eq!(
            total_withdrawn_after_staking_setup_call + expected_harvested_amount,
            investor_state.harvested
        );

        Ok(())
    }
}
