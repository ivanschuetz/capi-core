#[cfg(test)]
mod tests {
    use crate::{
        flows::{claim::claim::claimable_dividend, create_dao::share_amount::ShareAmount},
        funds::FundsAmount,
        state::dao_app_state::{dao_global_state, dao_investor_state},
        teal::TealSource,
        testing::{
            flow::{
                claim_flow::{claim_flow, test::claim_precs_with_dao},
                create_dao_flow::test::create_dao_flow_with_owner,
                update_dao_flow::update_dao_flow,
            },
            network_test_util::test_dao_init,
        },
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial]
    async fn test_update_works() -> Result<()> {
        let td = &test_dao_init().await?;

        // precs

        let owner = &td.creator;
        let dao = create_dao_flow_with_owner(td, &owner.address()).await?;

        // in this test we double-check that the update works, by being able to perform a tx that we were not able to perform before of the update.
        // the most simple tx to do this check is coincidentally updating: it's only 1 tx and the only check it's that it's signed by the owner.
        // so we:
        // 1) check that updating by non-owner doesn't work
        // 2) update to TEAL that always returns 1
        // 3) check that updating by non-owner works (since now TEAL accepts everything)

        // sanity: confirm that not_owner can't update
        let not_owner = &td.investor1; // arbitrary account that's not the owner
        let update_by_not_owner_res = update_dao_flow(
            td,
            &dao,
            &not_owner,
            always_accept_teal(),
            always_accept_teal(),
        )
        .await;
        log::debug!("update_by_not_owner_res: {update_by_not_owner_res:?}");
        assert!(update_by_not_owner_res.is_err());

        // test

        // update to "always accept"
        let update_by_owner_res =
            update_dao_flow(td, &dao, &owner, always_accept_teal(), always_accept_teal()).await;
        log::debug!("update_by_owner_res: {update_by_owner_res:?}");
        assert!(update_by_owner_res.is_ok());

        // update by non-owner: since new TEAL accepts everything, this time it passes
        let new_update_by_non_owner_res = update_dao_flow(
            td,
            &dao,
            &not_owner,
            // updating to different TEAL, just in case that upgrades are rejected if it's the same TEAL
            // this can be arbitrary TEAL - just has to be different to the previous one
            always_rejects_teal(),
            always_rejects_teal(),
        )
        .await;
        log::debug!("new_update_by_non_owner_res: {new_update_by_non_owner_res:?}");
        assert!(new_update_by_non_owner_res.is_ok());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_update_does_not_affect_state() -> Result<()> {
        let td = &test_dao_init().await?;

        // precs

        let owner = &td.creator;
        let dao = create_dao_flow_with_owner(td, &owner.address()).await?;

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

        // // flow

        let global_state_before_update = dao_global_state(&td.algod, dao.app_id).await?;
        let local_state_before_update =
            dao_investor_state(&td.algod, &investor.address(), dao.app_id).await?;

        let update_res =
            update_dao_flow(td, &dao, &owner, always_accept_teal(), always_accept_teal()).await;
        assert!(update_res.is_ok());

        // test

        let global_state_after_update = dao_global_state(&td.algod, dao.app_id).await?;
        log::debug!("global_state_after_update: {global_state_after_update:?}");
        let local_state_after_update =
            dao_investor_state(&td.algod, &investor.address(), dao.app_id).await?;

        assert_eq!(global_state_before_update, global_state_after_update);
        assert_eq!(local_state_before_update, local_state_after_update);

        Ok(())
    }

    fn always_accept_teal() -> TealSource {
        TealSource(
            r#"
        #pragma version 5
        int 1
        "#
            .as_bytes()
            .to_vec(),
        )
    }

    fn always_rejects_teal() -> TealSource {
        TealSource(
            r#"
        #pragma version 5
        int 0
        "#
            .as_bytes()
            .to_vec(),
        )
    }
}
