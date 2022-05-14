#[cfg(test)]
mod tests {
    use crate::{
        capi_asset::capi_app_state::capi_app_global_state,
        flows::create_dao::setup::customer_escrow,
        state::{account_state::funds_holdings, dao_app_state::dao_global_state},
        testing::{
            flow::{
                create_dao_flow::create_dao_flow,
                customer_payment_and_drain_flow::{customer_payment_and_drain_flow, drain_flow},
            },
            network_test_util::test_dao_init,
        },
    };
    use anyhow::Result;
    use mbase::models::funds::FundsAmount;
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

        let customer_escrow_balance = algod
            .account_information(drain_res.dao.customer_escrow.address())
            .await?
            .amount;
        let app_balance =
            funds_holdings(&algod, &drain_res.dao.app_address(), td.funds_asset_id).await?;
        let drainer_balance = algod.account_information(&drainer.address()).await?.amount;

        // account keeps min balance
        assert_eq!(customer_escrow::MIN_BALANCE, customer_escrow_balance);
        // dao central escrow has now the funds from customer escrow
        assert_eq!(drain_res.drained_amounts.dao, app_balance);
        // the drainer lost the fees for the app calls and escrows lsig
        assert_eq!(
            drain_res.initial_drainer_balance
                - drain_res.app_call_tx.fee // the app call pays its own fee and the escrow fees
                - drain_res.capi_app_call_tx.fee,
            drainer_balance
        );
        // capi escrow received its part
        let capi_escrow_amount =
            funds_holdings(&algod, &td.capi_app_id.address(), td.funds_asset_id).await?;
        assert_eq!(drain_res.drained_amounts.capi, capi_escrow_amount);

        // dao app received global state set to what was drained (to the dao)
        let dao_state = dao_global_state(&algod, dao.app_id).await?;
        assert_eq!(drain_res.drained_amounts.dao, dao_state.received);

        // capi app received global state set to what was drained (to capi)
        let capi_state = capi_app_global_state(&algod, td.capi_app_id).await?;
        assert_eq!(drain_res.drained_amounts.capi, capi_state.received);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_drain_fails_if_theres_nothing_to_drain() -> Result<()> {
        let td = &test_dao_init().await?;
        let drainer = &td.investor1;

        let dao = create_dao_flow(td).await?;

        // flow

        let drain_res = drain_flow(td, drainer, &dao).await;

        // tes

        assert!(drain_res.is_err());

        Ok(())
    }
}
