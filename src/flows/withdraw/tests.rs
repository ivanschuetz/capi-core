#[cfg(test)]
mod tests {
    use crate::{
        flows::withdraw::withdraw::{submit_withdraw, withdraw, WithdrawSigned, WithdrawalInputs},
        state::account_state::funds_holdings,
        testing::{
            create_and_submit_txs::transfer_tokens_submit,
            flow::{
                create_dao_flow::create_dao_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
                invest_in_dao_flow::{invests_flow, invests_optins_flow},
                withdraw_flow::{test::withdraw_flow, withdraw_precs},
            },
            network_test_util::{
                test_dao_init, test_dao_with_funds_target_init, test_dao_with_specs,
            },
            test_data::{dao_specs, dao_specs_with_funds_pars, investor2},
        },
    };
    use algonaut::{
        algod::v2::Algod,
        core::{to_app_address, Address},
    };
    use anyhow::Result;
    use chrono::{Duration, Utc};
    use mbase::{
        checked::{CheckedAdd, CheckedMulOther, CheckedSub},
        date_util::DateTimeExt,
        models::{
            funds::{FundsAmount, FundsAssetId},
            share_amount::ShareAmount,
        },
    };
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_withdraw_directly_sent_funds_fails() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;

        // precs

        let withdraw_amount = FundsAmount::new(1_000_000);

        let dao = create_dao_flow(&td).await?;

        let params = algod.suggested_transaction_params().await?;
        let dao_address = to_app_address(dao.app_id.0);
        log::debug!("app address: {dao_address}");

        // send funds ot dao
        // note that we're not draining this payment
        transfer_tokens_submit(
            algod,
            &params,
            &td.creator,
            &dao_address,
            td.funds_asset_id.0,
            withdraw_amount.0,
        )
        .await?;

        // flow

        let res = withdraw_flow(&algod, &dao, &td.creator, withdraw_amount, dao.app_id).await;

        // test

        assert!(res.is_err());

        Ok(())
    }

    #[test]
    #[serial]
    // in this test, we fund the dao (withdrawal precondition) via the normal user flow - investing and draining
    // (opposed to test_basic_withdraw_success)
    async fn test_withdraw_success() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let drainer = &td.investor1;

        // precs

        let withdraw_amount = FundsAmount::new(1_000_000);

        let dao = create_dao_flow(&td).await?;
        let pay_and_drain_amount = FundsAmount::new(10 * 1_000_000);

        withdraw_precs(td, drainer, &dao, pay_and_drain_amount).await?;

        // remeber state
        let app_balance_before_withdrawing =
            funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        let creator_balance_bafore_withdrawing =
            funds_holdings(&algod, &td.creator.address(), td.funds_asset_id).await?;

        // flow

        withdraw_flow(&algod, &dao, &td.creator, withdraw_amount, dao.app_id).await?;

        // test

        after_withdrawal_success_or_failure_tests(
            &algod,
            &td.creator.address(),
            td.funds_asset_id,
            &dao.app_address(),
            // creator got the amount
            creator_balance_bafore_withdrawing
                .add(&withdraw_amount)
                .unwrap(),
            // central lost the withdrawn amount
            app_balance_before_withdrawing
                .sub(&withdraw_amount)
                .unwrap(),
        )
        .await
    }

    #[test]
    #[serial]
    async fn test_withdraw_without_enough_funds_fails() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        // precs

        let dao_specs = dao_specs();
        let investor_share_amount = ShareAmount::new(10);

        let investment_amount = dao_specs
            .share_price
            .mul(investor_share_amount.val())
            .unwrap();

        let withdraw_amount = investment_amount.add(&FundsAmount::new(1)).unwrap(); // > investment amount (which is in the funds when withdrawing)

        let dao = create_dao_flow(td).await?;

        // Investor buys some shares
        invests_optins_flow(algod, &investor, &dao).await?;
        invests_flow(td, investor, investor_share_amount, &dao).await?;

        // remember state
        let app_balance_before_withdrawing =
            funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        let creator_balance_bafore_withdrawing =
            funds_holdings(algod, &td.creator.address(), td.funds_asset_id).await?;

        // flow

        let to_sign = withdraw(
            algod,
            td.creator.address(),
            &WithdrawalInputs {
                amount: withdraw_amount,
                description: "Withdrawing from tests".to_owned(),
            },
            dao.app_id,
            dao.funds_asset_id,
        )
        .await?;

        let withdraw_signed = td.creator.sign_transaction(to_sign.withdraw_tx)?;

        let withdraw_res = submit_withdraw(
            algod,
            &WithdrawSigned {
                withdraw_tx: withdraw_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            algod,
            &td.creator.address(),
            td.funds_asset_id,
            &dao.app_address(),
            creator_balance_bafore_withdrawing,
            app_balance_before_withdrawing,
        )
        .await
    }

    // TODO: test is failing after removing governance - add creator check to central escrow
    #[test]
    #[serial]
    async fn test_withdraw_by_not_creator_fails() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let drainer = &td.investor1;
        let investor = &td.investor2;
        let not_creator = &investor2();

        // precs

        let withdraw_amount = FundsAmount::new(1_000_000);

        let dao = create_dao_flow(&td).await?;
        let pay_and_drain_amount = FundsAmount::new(10 * 1_000_000);

        // customer payment and draining, to have some funds to withdraw
        customer_payment_and_drain_flow(td, &dao, pay_and_drain_amount, drainer).await?;

        // Investor buys some shares
        let investor_share_amount = ShareAmount::new(10);
        invests_optins_flow(algod, investor, &dao).await?;
        invests_flow(td, investor, investor_share_amount, &dao).await?;

        // remember state
        let app_balance_before_withdrawing =
            funds_holdings(algod, &dao.app_address(), td.funds_asset_id).await?;
        let creator_balance_bafore_withdrawing =
            funds_holdings(algod, &td.creator.address(), td.funds_asset_id).await?;

        // flow

        let to_sign = withdraw(
            algod,
            not_creator.address(),
            &WithdrawalInputs {
                amount: withdraw_amount,
                description: "Withdrawing from tests".to_owned(),
            },
            dao.app_id,
            dao.funds_asset_id,
        )
        .await?;

        let withdraw_signed = not_creator.sign_transaction(to_sign.withdraw_tx)?;

        let withdraw_res = submit_withdraw(
            algod,
            &WithdrawSigned {
                withdraw_tx: withdraw_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            algod,
            &td.creator.address(),
            td.funds_asset_id,
            &dao.app_address(),
            creator_balance_bafore_withdrawing,
            app_balance_before_withdrawing,
        )
        .await
    }

    async fn test_withdrawal_did_not_succeed(
        algod: &Algod,
        creator_address: &Address,
        funds_asset_id: FundsAssetId,
        app_address: &Address,
        creator_balance_before_withdrawing: FundsAmount,
        central_balance_before_withdrawing: FundsAmount,
    ) -> Result<()> {
        after_withdrawal_success_or_failure_tests(
            algod,
            creator_address,
            funds_asset_id,
            app_address,
            creator_balance_before_withdrawing,
            central_balance_before_withdrawing,
        )
        .await
    }

    async fn after_withdrawal_success_or_failure_tests(
        algod: &Algod,
        creator_address: &Address,
        funds_asset_id: FundsAssetId,
        app_address: &Address,
        expected_withdrawer_amount: FundsAmount,
        expected_central_amount: FundsAmount,
    ) -> Result<()> {
        // check creator's balance
        let withdrawer_amount = funds_holdings(algod, creator_address, funds_asset_id).await?;
        assert_eq!(expected_withdrawer_amount, withdrawer_amount);

        // check app's balance
        let central_escrow_amount = funds_holdings(algod, app_address, funds_asset_id).await?;
        assert_eq!(expected_central_amount, central_escrow_amount);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cannot_withdraw_before_funds_target_finished() -> Result<()> {
        // the deps have a target raise end date at some point in the future,
        // we try to withdraw "now" - can't, because it's not possible to withdraw funds before the project has finished raising the min target,
        // and we're before the end date
        let td = &test_dao_with_funds_target_init().await?;
        let algod = &td.algod;

        // precs

        let withdraw_amount = FundsAmount::new(1_000_000);

        let dao = create_dao_flow(&td).await?;

        let params = algod.suggested_transaction_params().await?;
        let dao_address = to_app_address(dao.app_id.0);
        log::debug!("app address: {dao_address}");

        // send asset to the DAO app
        transfer_tokens_submit(
            algod,
            &params,
            &td.creator,
            &dao_address,
            td.funds_asset_id.0,
            withdraw_amount.0,
        )
        .await?;

        // flow

        let res = withdraw_flow(&algod, &dao, &td.creator, withdraw_amount, dao.app_id).await;

        // test

        println!("res: {res:?}");
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cannot_withdraw_after_funds_target_end_date_if_target_wasnt_raised() -> Result<()>
    {
        test_can_withdraw_after_funds_target_end_date(
            FundsAmount::new(100_000_000),
            FundsAmount::new(5_000_000),
            ShareAmount::new(10), // * price = 50_000_000, which is less than funds target -> can't withdraw
            false,
        )
        .await
    }

    #[test]
    #[serial]
    // note that this test is essentially not different to the basic withdrawal tests,
    // where the funds target parameters allow the withdrawal to succeed
    // just for completeness, to have a counter part for test_cannot_withdraw_after_funds_target_end_date_if_target_wasnt_raised
    // where we test with a realistic funds target (not 0)
    async fn test_can_withdraw_after_funds_target_end_date_if_target_was_raised() -> Result<()> {
        test_can_withdraw_after_funds_target_end_date(
            FundsAmount::new(100_000_000),
            FundsAmount::new(5_000_000),
            ShareAmount::new(80), // * price = 400_000_000, which is more than funds target -> can withdraw
            true,
        )
        .await
    }

    async fn test_can_withdraw_after_funds_target_end_date(
        funds_target: FundsAmount,
        share_price: FundsAmount,
        buy_share_amount: ShareAmount,
        can_withdraw: bool,
    ) -> Result<()> {
        // specs with:
        // funds amount > 0 (so there have to be investments before withdrawing),
        // end date in the past (so raising is already finished - teal compares with a "now" - generated in teal)
        let funds_end_date = Utc::now() - Duration::minutes(1);
        let mut specs = dao_specs_with_funds_pars(funds_target, funds_end_date.to_timestap());
        // ensure that share price meets test conditions
        specs.share_price = share_price;

        let td = &test_dao_with_specs(&specs).await?;
        let algod = &td.algod;
        let investor = &td.investor1;

        // precs

        let dao = create_dao_flow(td).await?;

        // funds raising: an investor buys shares
        invests_optins_flow(algod, &investor, &dao).await?;
        invests_flow(td, investor, buy_share_amount, &dao).await?;

        // remeber state
        let app_balance_before_withdrawing =
            funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;

        // sanity check: the app has the funds raised
        assert_eq!(
            app_balance_before_withdrawing,
            FundsAmount::new(buy_share_amount.val() * specs.share_price.val())
        );

        // flow

        let withdraw_amount = FundsAmount::new(1_000_000);
        let res = withdraw_flow(&algod, &dao, &td.creator, withdraw_amount, dao.app_id).await;

        // test

        println!("res: {res:?}");

        if can_withdraw {
            assert!(res.is_ok());
        } else {
            assert!(res.is_err());
        }

        Ok(())
    }
}
