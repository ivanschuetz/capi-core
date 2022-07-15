#[cfg(test)]
mod tests {
    use crate::{
        state::account_state::funds_holdings,
        testing::{
            flow::{
                create_dao_flow::create_dao_flow,
                customer_payment_and_drain_flow::{customer_payment_and_drain_flow, drain_flow},
                withdraw_flow::test::withdraw_flow,
            },
            network_test_util::test_dao_init,
        },
    };
    use anyhow::Result;
    use mbase::{models::funds::FundsAmount, state::dao_app_state::dao_global_state};
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_drain() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;
        let drainer = &td.investor1;

        let dao = create_dao_flow(&td).await?;

        let customer_payment_amount = FundsAmount::new(10 * 1_000_000);

        // flow

        let drain_res =
            customer_payment_and_drain_flow(&td, &dao, customer_payment_amount, drainer).await?;

        // test

        let app_balance = funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        let drainer_balance = algod.account_information(&drainer.address()).await?.amount;

        // check that the app still has the funds, minus capi fee
        assert_eq!(drain_res.drained_amounts.dao, app_balance);

        // withdrawable amount global state was set to drained funds
        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        assert_eq!(drain_res.drained_amounts.dao, dao_state.available);
        // total received received global state was incremented with what we drained (minus capi fee)
        assert_eq!(drain_res.drained_amounts.dao, dao_state.received);

        // the drainer lost the fee
        assert_eq!(
            drain_res.initial_drainer_balance - drain_res.app_call_tx.fee,
            drainer_balance
        );

        // capi escrow received the capi fee
        let capi_escrow_amount =
            funds_holdings(&algod, &td.capi_address.0, td.funds_asset_id).await?;
        assert_eq!(drain_res.drained_amounts.capi, capi_escrow_amount);

        Ok(())
    }

    // buys shares - drains - buys shares - drains
    #[test]
    #[serial]
    async fn test_buy_shares_and_drain_twice() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;
        let drainer = &td.investor1;

        let dao = create_dao_flow(&td).await?;

        let customer_payment_amount = FundsAmount::new(10 * 1_000_000);

        // flow

        let drain_res1 =
            customer_payment_and_drain_flow(&td, &dao, customer_payment_amount, drainer).await?;

        let customer_payment_amount2 = FundsAmount::new(50 * 1_000_000);

        let drain_res2 =
            customer_payment_and_drain_flow(&td, &dao, customer_payment_amount2, drainer).await?;

        // tests

        let app_balance = funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        let drainer_balance = algod.account_information(&drainer.address()).await?.amount;

        // check that the app still has the funds, minus capi fee
        let expected_dao_funds = FundsAmount::new(
            drain_res1.drained_amounts.dao.val() + drain_res2.drained_amounts.dao.val(),
        );
        assert_eq!(expected_dao_funds, app_balance);

        // withdrawable amount global state was set to drained funds total, minus capi fee
        // (i.e. the total received funds minus capi fee are available for withdrawal)
        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        assert_eq!(expected_dao_funds, dao_state.available);
        // total received received global state was incremented with the total drained (minus capi fee)
        assert_eq!(expected_dao_funds, dao_state.received);

        // the drainer lost fees
        let total_paid_fees = drain_res1.app_call_tx.fee + drain_res2.app_call_tx.fee;
        assert_eq!(
            drain_res1.initial_drainer_balance - total_paid_fees,
            drainer_balance
        );

        // capi escrow received the capi fees
        let total_received_capi_fees = FundsAmount::new(
            drain_res1.drained_amounts.capi.val() + drain_res2.drained_amounts.capi.val(),
        );
        let capi_escrow_amount =
            funds_holdings(&algod, &td.capi_address.0, td.funds_asset_id).await?;
        assert_eq!(total_received_capi_fees, capi_escrow_amount);

        Ok(())
    }

    // buys shares - drains - drains
    #[test]
    #[serial]
    async fn test_drain_twice() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;
        let drainer = &td.investor1;

        let dao = create_dao_flow(&td).await?;

        let customer_payment_amount = FundsAmount::new(10 * 1_000_000);

        // flow

        let drain_res1 =
            customer_payment_and_drain_flow(&td, &dao, customer_payment_amount, drainer).await?;

        let drain_res2 = drain_flow(&td, drainer, &dao).await?;

        // tests

        let app_balance = funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        let drainer_balance = algod.account_information(&drainer.address()).await?.amount;

        // double check that the second drain doesn't drain anything, because there has been no new income
        assert_eq!(FundsAmount::new(0), drain_res2.drained_amounts.dao);
        assert_eq!(FundsAmount::new(0), drain_res2.drained_amounts.capi);

        // check that the app still has the funds, minus capi fee
        let expected_dao_funds = FundsAmount::new(
            drain_res1.drained_amounts.dao.val() + drain_res2.drained_amounts.dao.val(),
        );
        assert_eq!(expected_dao_funds, app_balance);

        // withdrawable amount global state was set to drained funds total, minus capi fee
        // (i.e. the total received funds minus capi fee are available for withdrawal)
        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        assert_eq!(expected_dao_funds, dao_state.available);
        // total received received global state was incremented with the total drained (minus capi fee)
        assert_eq!(expected_dao_funds, dao_state.received);

        // the drainer lost fees
        let total_paid_fees = drain_res1.app_call_tx.fee + drain_res2.app_call_tx.fee;
        assert_eq!(
            drain_res1.initial_drainer_balance - total_paid_fees,
            drainer_balance
        );

        // capi escrow received the capi fees
        let total_received_capi_fees = FundsAmount::new(
            drain_res1.drained_amounts.capi.val() + drain_res2.drained_amounts.capi.val(),
        );
        let capi_escrow_amount =
            funds_holdings(&algod, &td.capi_address.0, td.funds_asset_id).await?;
        assert_eq!(total_received_capi_fees, capi_escrow_amount);

        Ok(())
    }

    // invest - drain - withdraw - drain
    #[test]
    #[serial]
    async fn test_drain_withdraw_drain() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;
        let drainer = &td.investor1;

        let dao = create_dao_flow(&td).await?;

        let customer_payment_amount = FundsAmount::new(10 * 1_000_000);

        // flow

        let drain_res1 =
            customer_payment_and_drain_flow(&td, &dao, customer_payment_amount, drainer).await?;

        // withdraw something between drains

        // remeber state
        let withdrawer_funds_before_withdrawing =
            funds_holdings(&algod, &td.creator.address(), td.funds_asset_id).await?;

        let withdraw_amount = FundsAmount::new(5 * 1_000_000);
        withdraw_flow(&algod, &dao, &td.creator, withdraw_amount, dao.app_id).await?;

        let drain_res2 = drain_flow(&td, drainer, &dao).await?;

        // tests

        // double check that the second drain doesn't drain anything, because there has been no new income
        assert_eq!(FundsAmount::new(0), drain_res2.drained_amounts.dao);
        assert_eq!(FundsAmount::new(0), drain_res2.drained_amounts.capi);

        let app_balance = funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        let drainer_balance = algod.account_information(&drainer.address()).await?.amount;

        // check that the app has the funds, minus capi fee and the withdrawal
        let expected_dao_funds = FundsAmount::new(
            drain_res1.drained_amounts.dao.val() + drain_res2.drained_amounts.dao.val()
                - withdraw_amount.val(),
        );
        assert_eq!(expected_dao_funds, app_balance);

        // withdrawable amount global state was set to drained funds total, minus capi fee, minus withdrawal
        // (i.e. the total received funds minus capi fee are available for withdrawal)
        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        assert_eq!(expected_dao_funds, dao_state.available);
        // total received received global state was incremented with the total drained (minus capi fee, minus withdrawal)
        let expected_dao_total_received = FundsAmount::new(
            drain_res1.drained_amounts.dao.val() + drain_res2.drained_amounts.dao.val(),
        );
        assert_eq!(expected_dao_total_received, dao_state.received);

        // the drainer lost fees
        let total_paid_fees = drain_res1.app_call_tx.fee + drain_res2.app_call_tx.fee;
        assert_eq!(
            drain_res1.initial_drainer_balance - total_paid_fees,
            drainer_balance
        );

        // capi escrow received the capi fees
        let total_received_capi_fees = FundsAmount::new(
            drain_res1.drained_amounts.capi.val() + drain_res2.drained_amounts.capi.val(),
        );
        let capi_escrow_amount =
            funds_holdings(&algod, &td.capi_address.0, td.funds_asset_id).await?;
        assert_eq!(total_received_capi_fees, capi_escrow_amount);

        // withdrawer received the withdrawn amount
        let withdrawer_funds =
            funds_holdings(&algod, &td.creator.address(), td.funds_asset_id).await?;
        let expected_withdrawer_funds =
            FundsAmount::new(withdraw_amount.val() + withdrawer_funds_before_withdrawing.val());
        assert_eq!(expected_withdrawer_funds, withdrawer_funds);

        Ok(())
    }

    #[test]
    #[serial]
    // (no-op aside of wasting the txs fee)
    async fn test_drain_is_no_op_if_there_are_no_funds() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;
        let drainer = &td.investor1;

        let dao = create_dao_flow(td).await?;

        // flow

        let drain_res = drain_flow(td, drainer, &dao).await?;

        // tests

        // nothing to drain
        assert_eq!(FundsAmount::new(0), drain_res.drained_amounts.dao);
        assert_eq!(FundsAmount::new(0), drain_res.drained_amounts.capi);

        let app_balance = funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
        let drainer_balance = algod.account_information(&drainer.address()).await?.amount;

        // apps still has no funds
        assert_eq!(FundsAmount::new(0), app_balance);

        let dao_state = dao_global_state(&algod, dao.app_id).await?;

        // nothing drained or invested: no withdrawable amount
        assert_eq!(FundsAmount::new(0), dao_state.available);

        // nothing received: total received is 0
        assert_eq!(FundsAmount::new(0), dao_state.received);

        // the drainer paid the fees
        assert_eq!(
            drain_res.initial_drainer_balance - drain_res.app_call_tx.fee,
            drainer_balance
        );

        // nothing drained: capi escrow didn't get any fees
        let capi_escrow_amount =
            funds_holdings(&algod, &td.capi_address.0, td.funds_asset_id).await?;
        assert_eq!(FundsAmount::new(0), capi_escrow_amount);

        Ok(())
    }
}
