#[cfg(test)]
mod tests {
    use crate::{
        capi_asset::{
            capi_app_state::{capi_app_global_state, capi_app_investor_state},
            capi_asset_id::CapiAssetAmount,
            claim::claim::claimable_capi_dividend,
        },
        funds::FundsAmount,
        teal::TealSource,
        testing::{
            flow::{
                claim_capi_flow::{claim_capi_flow, claim_capi_precs},
                update_capi_flow::update_capi_flow,
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

        // in this test we double-check that the update works, by being able to perform a tx that we were not able to perform before of the update.
        // the most simple tx to do this check is coincidentally updating: it's only 1 tx and the only check it's that it's signed by the owner.
        // so we:
        // 1) check that updating by non-owner doesn't work
        // 2) update to TEAL that always returns 1
        // 3) check that updating by non-owner works (since now TEAL accepts everything)

        // sanity: confirm that not_owner can't update
        let not_owner = &td.investor1; // arbitrary account that's not the owner
        let update_by_not_owner_res = update_capi_flow(
            td,
            td.capi_app_id,
            &not_owner,
            always_accept_teal(),
            always_accept_teal(),
        )
        .await;
        log::debug!("update_by_not_owner_res: {update_by_not_owner_res:?}");
        assert!(update_by_not_owner_res.is_err());

        // test

        // update to "always accept"
        let update_by_owner_res = update_capi_flow(
            td,
            td.capi_app_id,
            &td.capi_owner,
            always_accept_teal(),
            always_accept_teal(),
        )
        .await;
        log::debug!("update_by_owner_res: {update_by_owner_res:?}");
        assert!(update_by_owner_res.is_ok());

        // update by non-owner: since new TEAL accepts everything, this time it passes
        let new_update_by_non_owner_res = update_capi_flow(
            td,
            td.capi_app_id,
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
        let algod = &td.algod;

        // precs

        // invest and claim some dividend - after this all the app's global and local variables should be set to something

        let investor = &td.investor1;

        let investor_capi_amount = CapiAssetAmount::new(100_000);
        let initial_capi_funds_amount = FundsAmount::new(200_000);
        let precs = claim_capi_precs(
            td,
            &td.capi_owner,
            investor,
            investor_capi_amount,
            initial_capi_funds_amount,
        )
        .await?;
        let dividend = claimable_capi_dividend(
            // the calculated capi fee is what's on the capi app (total received state) now
            precs.drain_res.drained_amounts.capi,
            FundsAmount::new(0),
            investor_capi_amount,
            td.capi_supply,
            td.precision,
        )?;
        claim_capi_flow(
            algod,
            investor,
            dividend,
            td.funds_asset_id,
            td.capi_app_id,
            &td.capi_escrow.account,
        )
        .await?;

        // flow

        let global_state_before_update = capi_app_global_state(&td.algod, td.capi_app_id).await?;
        let local_state_before_update =
            capi_app_investor_state(&td.algod, &investor.address(), td.capi_app_id).await?;

        let update_res = update_capi_flow(
            td,
            td.capi_app_id,
            &td.capi_owner,
            always_accept_teal(),
            always_accept_teal(),
        )
        .await;
        assert!(update_res.is_ok());

        // test

        let global_state_after_update = capi_app_global_state(&td.algod, td.capi_app_id).await?;
        let local_state_after_update =
            capi_app_investor_state(&td.algod, &investor.address(), td.capi_app_id).await?;

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
