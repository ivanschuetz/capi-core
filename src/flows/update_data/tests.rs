#[cfg(test)]
mod tests {
    use crate::{
        api::version::{Version, VersionedAddress},
        flows::{
            claim::claim::claimable_dividend, create_dao::share_amount::ShareAmount,
            update_data::update_data::UpdatableDaoData,
        },
        funds::FundsAmount,
        state::dao_app_state::{dao_global_state, dao_investor_state, CentralAppGlobalState},
        testing::{
            flow::{
                claim_flow::{claim_flow, test::claim_precs_with_dao},
                create_dao_flow::test::create_dao_flow_with_owner,
                update_dao_data_flow::update_dao_data_flow,
            },
            network_test_util::{test_dao_init, TestDeps},
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

        let update_data = some_data_to_update(&td);

        // precs

        // sanity check: current state is different to the new one
        let gs_before_update = dao_global_state(algod, dao.app_id).await?;
        sanity_check_current_state_different_to_update_data(&gs_before_update, &update_data);

        // flow

        update_dao_data_flow(td, &dao, &owner, &update_data).await?;

        // test

        let gs_after_update = dao_global_state(algod, dao.app_id).await?;
        validate_global_state_with_update_data(&gs_after_update, &update_data);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_update_data_does_not_affect_other_state() -> Result<()> {
        let td = &test_dao_init().await?;
        let algod = &td.algod;

        let owner = &td.creator;
        let dao = create_dao_flow_with_owner(td, &owner.address()).await?;

        let update_data = some_data_to_update(&td);

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

        // sanity check: current state is different to the new one
        sanity_check_current_state_different_to_update_data(&gs_before_update, &update_data);

        // flow

        update_dao_data_flow(td, &dao, &owner, &update_data).await?;

        // test

        let gs_after_update = dao_global_state(algod, dao.app_id).await?;
        let ls_after_update =
            dao_investor_state(&td.algod, &investor.address(), dao.app_id).await?;

        validate_global_state_with_update_data(&gs_after_update, &update_data);

        // aside of what we updated, data.global state stays the same
        assert_eq!(
            gs_after_update.funds_asset_id,
            gs_before_update.funds_asset_id
        );
        assert_eq!(gs_after_update.received, gs_before_update.received);
        assert_eq!(
            gs_after_update.shares_asset_id,
            gs_before_update.shares_asset_id
        );

        // local state stays the same
        assert_eq!(ls_before_update, ls_after_update);

        Ok(())
    }

    fn some_data_to_update(td: &TestDeps) -> UpdatableDaoData {
        // arbitrary data different to the existing one
        let new_central_escrow_address = td.investor1.address();
        let new_customer_escrow_address = td.investor2.address();
        let new_investing_escrow_address = td.creator.address();
        let new_locking_escrow_address = td.capi_owner.address();
        let new_project_name = "new_project_name".to_owned();
        let new_project_desc = "new_project_desc".to_owned();
        let new_share_price = FundsAmount::new(121212);
        let new_logo_url = "new_logo_url".to_owned();
        let new_social_media_url = "new_social_media_url".to_owned();
        let new_owner = td.customer.address();

        UpdatableDaoData {
            central_escrow: VersionedAddress::new(new_central_escrow_address, Version(2)),
            customer_escrow: VersionedAddress::new(new_customer_escrow_address, Version(2)),
            investing_escrow: VersionedAddress::new(new_investing_escrow_address, Version(2)),
            locking_escrow: VersionedAddress::new(new_locking_escrow_address, Version(2)),
            project_name: new_project_name.clone(),
            project_desc: new_project_desc.clone(),
            share_price: new_share_price,
            logo_url: new_logo_url.clone(),
            social_media_url: new_social_media_url.clone(),
            owner: new_owner,
        }
    }

    fn validate_global_state_with_update_data(gs: &CentralAppGlobalState, data: &UpdatableDaoData) {
        assert_eq!(gs.central_escrow, data.central_escrow);
        assert_eq!(gs.customer_escrow, data.customer_escrow);
        assert_eq!(gs.investing_escrow, data.investing_escrow);
        assert_eq!(gs.locking_escrow, data.locking_escrow);
        assert_eq!(gs.project_name, data.project_name);
        assert_eq!(gs.project_desc, data.project_desc);
        assert_eq!(gs.share_price, data.share_price);
        assert_eq!(gs.logo_url, data.logo_url);
        assert_eq!(gs.social_media_url, data.social_media_url);
        assert_eq!(gs.owner, data.owner);
    }

    fn sanity_check_current_state_different_to_update_data(
        gs: &CentralAppGlobalState,
        data: &UpdatableDaoData,
    ) {
        assert_ne!(gs.central_escrow, data.central_escrow);
        assert_ne!(gs.customer_escrow, data.customer_escrow);
        assert_ne!(gs.investing_escrow, data.investing_escrow);
        assert_ne!(gs.locking_escrow, data.locking_escrow);
        assert_ne!(gs.project_name, data.project_name);
        assert_ne!(gs.project_desc, data.project_desc);
        assert_ne!(gs.share_price, data.share_price);
        assert_ne!(gs.logo_url, data.logo_url);
        assert_ne!(gs.social_media_url, data.social_media_url);
        assert_ne!(gs.owner, data.owner);
    }
}
