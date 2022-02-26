#[cfg(test)]
mod tests {
    use algonaut::{algod::v2::Algod, core::Address};
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::{
            create_project::share_amount::ShareAmount,
            withdraw::withdraw::{submit_withdraw, withdraw, WithdrawSigned, WithdrawalInputs},
        },
        funds::{FundsAmount, FundsAssetId},
        state::account_state::funds_holdings,
        testing::{
            flow::{
                create_project_flow::create_project_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
                invest_in_project_flow::{invests_flow, invests_optins_flow},
                withdraw_flow::{withdraw_flow, withdraw_precs},
            },
            network_test_util::{setup_on_chain_deps, test_init, OnChainDeps},
            test_data::{creator, customer, investor1, investor2, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
    };

    #[test]
    #[serial]
    async fn test_withdraw_success() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let drainer = investor1();
        let customer = customer();
        let OnChainDeps {
            funds_asset_id,
            capi_deps,
        } = setup_on_chain_deps(&algod).await?;

        // precs

        let withdraw_amount = FundsAmount(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        let pay_and_drain_amount = FundsAmount(10 * 1_000_000);

        withdraw_precs(
            &algod,
            &drainer,
            &customer,
            &project.project,
            pay_and_drain_amount,
            funds_asset_id,
            &capi_deps,
        )
        .await?;

        // remeber state
        let central_balance_before_withdrawing = funds_holdings(
            &algod,
            project.project.central_escrow.address(),
            funds_asset_id,
        )
        .await?;
        let creator_balance_bafore_withdrawing =
            funds_holdings(&algod, &creator.address(), funds_asset_id).await?;

        // flow

        withdraw_flow(
            &algod,
            &project.project,
            &creator,
            withdraw_amount,
            funds_asset_id,
        )
        .await?;

        // test

        after_withdrawal_success_or_failure_tests(
            &algod,
            &creator.address(),
            funds_asset_id,
            project.project.central_escrow.address(),
            // creator got the amount
            creator_balance_bafore_withdrawing + withdraw_amount,
            // central lost the withdrawn amount
            central_balance_before_withdrawing - withdraw_amount,
        )
        .await
    }

    #[test]
    #[serial]
    async fn test_withdraw_without_enough_funds_fails() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let investor = investor1();

        let funds_asset_id = setup_on_chain_deps(&algod).await?.funds_asset_id;

        // precs

        let project_specs = project_specs();
        let investor_share_amount = ShareAmount(10);

        let investment_amount = project_specs.share_price * investor_share_amount.0;

        let withdraw_amount = investment_amount + FundsAmount(1); // > investment amount (which is in the funds when withdrawing)

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs,
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;

        // Investor buys some shares
        invests_optins_flow(&algod, &investor, &project.project).await?;
        invests_flow(
            &algod,
            &investor,
            investor_share_amount,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // remember state
        let central_balance_before_withdrawing = funds_holdings(
            &algod,
            project.project.central_escrow.address(),
            funds_asset_id,
        )
        .await?;
        let creator_balance_bafore_withdrawing =
            funds_holdings(&algod, &creator.address(), funds_asset_id).await?;

        // flow

        let to_sign = withdraw(
            &algod,
            creator.address(),
            funds_asset_id,
            &WithdrawalInputs {
                amount: withdraw_amount,
                description: "Withdrawing from tests".to_owned(),
            },
            &project.project.central_escrow,
        )
        .await?;

        // UI
        let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;

        let withdraw_res = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            &algod,
            &creator.address(),
            funds_asset_id,
            project.project.central_escrow.address(),
            creator_balance_bafore_withdrawing,
            central_balance_before_withdrawing,
        )
        .await
    }

    // TODO: test is failing after removing governance - add creator check to central escrow
    #[test]
    #[serial]
    async fn test_withdraw_by_not_creator_fails() -> Result<()> {
        test_init()?;

        // deps

        let algod = dependencies::algod_for_tests();
        let creator = creator();
        let drainer = investor1();
        let investor = investor2();
        let customer = customer();
        let not_creator = investor2();
        let OnChainDeps {
            funds_asset_id,
            capi_deps,
        } = setup_on_chain_deps(&algod).await?;

        // precs

        let withdraw_amount = FundsAmount(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        let pay_and_drain_amount = FundsAmount(10 * 1_000_000);

        // customer payment and draining, to have some funds to withdraw
        customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            funds_asset_id,
            pay_and_drain_amount,
            &project.project,
            &capi_deps,
        )
        .await?;

        // Investor buys some shares
        let investor_share_amount = ShareAmount(10);
        invests_optins_flow(&algod, &investor, &project.project).await?;
        invests_flow(
            &algod,
            &investor,
            investor_share_amount,
            funds_asset_id,
            &project.project,
            &project.project_id,
        )
        .await?;

        // remember state
        let central_balance_before_withdrawing = funds_holdings(
            &algod,
            project.project.central_escrow.address(),
            funds_asset_id,
        )
        .await?;
        let creator_balance_bafore_withdrawing =
            funds_holdings(&algod, &creator.address(), funds_asset_id).await?;

        // flow

        let to_sign = withdraw(
            &algod,
            not_creator.address(),
            funds_asset_id,
            &WithdrawalInputs {
                amount: withdraw_amount,
                description: "Withdrawing from tests".to_owned(),
            },
            &project.project.central_escrow,
        )
        .await?;

        // UI
        let pay_withdraw_fee_tx_signed =
            not_creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;

        let withdraw_res = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            &algod,
            &creator.address(),
            funds_asset_id,
            project.project.central_escrow.address(),
            creator_balance_bafore_withdrawing,
            central_balance_before_withdrawing,
        )
        .await
    }

    async fn test_withdrawal_did_not_succeed(
        algod: &Algod,
        creator_address: &Address,
        funds_asset_id: FundsAssetId,
        central_escrow_address: &Address,
        creator_balance_before_withdrawing: FundsAmount,
        central_balance_before_withdrawing: FundsAmount,
    ) -> Result<()> {
        after_withdrawal_success_or_failure_tests(
            algod,
            creator_address,
            funds_asset_id,
            central_escrow_address,
            creator_balance_before_withdrawing,
            central_balance_before_withdrawing,
        )
        .await
    }

    async fn after_withdrawal_success_or_failure_tests(
        algod: &Algod,
        creator_address: &Address,
        funds_asset_id: FundsAssetId,
        central_escrow_address: &Address,
        expected_withdrawer_amount: FundsAmount,
        expected_central_amount: FundsAmount,
    ) -> Result<()> {
        // check creator's balance
        let withdrawer_amount = funds_holdings(algod, creator_address, funds_asset_id).await?;
        assert_eq!(expected_withdrawer_amount, withdrawer_amount);

        // check central's balance
        let central_escrow_amount =
            funds_holdings(algod, central_escrow_address, funds_asset_id).await?;
        assert_eq!(expected_central_amount, central_escrow_amount);

        Ok(())
    }
}
