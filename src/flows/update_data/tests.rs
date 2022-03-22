#[cfg(test)]
mod tests {
    use crate::{
        flows::{
            claim::claim::claimable_dividend, create_dao::share_amount::ShareAmount,
            update_data::update_data::UpdatableDaoData,
        },
        funds::FundsAmount,
        state::central_app_state::{dao_global_state, dao_investor_state},
        testing::{
            flow::{
                claim_flow::{claim_flow, test::claim_precs_with_dao},
                create_dao_flow::test::create_dao_flow_with_owner,
                update_dao_data_flow::update_dao_data_flow,
            },
            network_test_util::test_dao_init,
        },
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_update_data_works() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;

        let owner = &td.creator;
        let dao = create_dao_flow_with_owner(td, &owner.address()).await?;

        // arbitrary data different to the existing one
        let new_central_escrow_address = td.investor1.address();
        let new_customer_escrow_address = td.investor2.address();

        // precs

        // sanity check: current state is different to the new one
        let global_state_before_update = dao_global_state(algod, dao.app_id).await?;
        assert_ne!(
            global_state_before_update.central_escrow,
            new_central_escrow_address
        );
        assert_ne!(
            global_state_before_update.customer_escrow,
            new_customer_escrow_address
        );

        // flow

        let data = UpdatableDaoData {
            central_escrow: new_central_escrow_address,
            customer_escrow: new_customer_escrow_address,
        };
        update_dao_data_flow(td, &dao, &owner, &data).await?;

        // test

        let global_state_after_update = dao_global_state(algod, dao.app_id).await?;
        assert_eq!(
            global_state_after_update.central_escrow,
            new_central_escrow_address
        );
        assert_eq!(
            global_state_after_update.customer_escrow,
            new_customer_escrow_address
        );

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_update_data_does_not_affect_other_state() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;

        let owner = &td.creator;
        let dao = create_dao_flow_with_owner(td, &owner.address()).await?;

        // arbitrary data different to the existing one
        let new_central_escrow_address = td.investor1.address();
        let new_customer_escrow_address = td.investor2.address();

        // precs

        // invest and claim some dividend - after this all the app's global and local variables should be set to something
        let investor = &td.investor2;
        let drainer = &td.investor1;
        let buy_share_amount = ShareAmount::new(10);
        let pay_and_drain_amount = FundsAmount::new(10_000_000);
        let precs = claim_precs_with_dao(
            &td,
            &dao,
            buy_share_amount,
            pay_and_drain_amount,
            drainer,
            investor,
        )
        .await?;
        let dividend = claimable_dividend(
            precs.drain_res.drained_amounts.dao,
            FundsAmount::new(0),
            td.specs.shares.supply,
            buy_share_amount,
            td.precision,
            td.specs.investors_part(),
        )?;
        claim_flow(&td, &precs.dao, investor, dividend).await?;

        let gs_before_update = dao_global_state(&td.algod, dao.app_id).await?;
        let ls_before_update =
            dao_investor_state(&td.algod, &investor.address(), dao.app_id).await?;

        // flow

        let data = UpdatableDaoData {
            central_escrow: new_central_escrow_address,
            customer_escrow: new_customer_escrow_address,
        };
        update_dao_data_flow(td, &dao, &owner, &data).await?;

        // test

        let gs_after_update = dao_global_state(algod, dao.app_id).await?;
        let ls_after_update =
            dao_investor_state(&td.algod, &investor.address(), dao.app_id).await?;

        // state was updated
        assert_eq!(gs_after_update.central_escrow, new_central_escrow_address);
        assert_eq!(gs_after_update.customer_escrow, new_customer_escrow_address);

        // aside of what we updated, global state stays the same
        assert_eq!(
            gs_before_update.funds_asset_id,
            gs_after_update.funds_asset_id
        );
        assert_eq!(gs_before_update.received, gs_after_update.received);
        assert_eq!(
            gs_before_update.shares_asset_id,
            gs_after_update.shares_asset_id
        );

        // local state stays the same
        assert_eq!(ls_before_update, ls_after_update);

        Ok(())
    }
}
