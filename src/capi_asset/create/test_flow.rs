#[cfg(test)]
pub mod test_flow {
    use crate::capi_asset::capi_app_id::CapiAppId;
    use crate::capi_asset::capi_asset_id::{CapiAssetAmount, CapiAssetId};
    use crate::capi_asset::create::create_capi_app::create_app;
    use crate::capi_asset::create::create_capi_asset::create_capi_asset;
    use crate::capi_asset::create::setup_capi_escrow::{
        setup_capi_escrow, submit_setup_capi_escrow, SetupCentralEscrowSigned,
    };
    use crate::funds::FundsAssetId;
    use crate::network_util::wait_for_pending_transaction;
    use crate::testing::flow::create_project_flow::capi_programs;
    use crate::testing::TESTS_DEFAULT_PRECISION;
    use algonaut::algod::v2::Algod;
    use algonaut::transaction::account::Account;
    use algonaut::transaction::contract_account::ContractAccount;
    use anyhow::Result;

    pub async fn setup_capi_asset_flow(
        algod: &Algod,
        creator: &Account,
        capi_supply: CapiAssetAmount,
        funds_asset_id: FundsAssetId,
    ) -> Result<CapiAssetFlowRes> {
        // create asset
        let params = algod.suggested_transaction_params().await?;

        let to_sign = create_capi_asset(capi_supply, &creator.address(), &params).await?;
        let signed = creator.sign_transaction(&to_sign.create_capi_asset_tx)?;
        log::debug!("Will submit crate capi asset..");
        let res = algod.broadcast_signed_transaction(&signed).await?;
        let p_tx = wait_for_pending_transaction(&algod, &res.tx_id.parse()?).await?;
        assert!(p_tx.is_some());
        let asset_id_opt = p_tx.unwrap().asset_index;
        assert!(asset_id_opt.is_some());
        let asset_id = CapiAssetId(asset_id_opt.unwrap());

        // create app

        let programs = capi_programs()?;
        let to_sign_app = create_app(
            &algod,
            &programs.app_approval,
            &programs.app_clear,
            &creator.address(),
            capi_supply,
            TESTS_DEFAULT_PRECISION,
            &params,
            asset_id,
            funds_asset_id,
        )
        .await?;
        let signed = creator.sign_transaction(&to_sign_app)?;
        log::debug!("Will submit crate capi app..");
        // crate::teal::debug_teal_rendered(&[signed.clone()], "app_capi_approval").unwrap();
        let res = algod.broadcast_signed_transaction(&signed).await?;
        let p_tx = wait_for_pending_transaction(&algod, &res.tx_id.parse()?).await?;
        assert!(p_tx.is_some());
        let app_id_opt = p_tx.unwrap().application_index;
        assert!(app_id_opt.is_some());
        let app_id = CapiAppId(app_id_opt.unwrap());

        // setup capi escrow

        // Note that here we create the app first and then the funds escrow.
        // In the DAOs, we do it the other way - it most likely will be changed.
        // It makes more sense for the escrow to know the app than the other way around.

        let to_sign = setup_capi_escrow(
            &algod,
            &creator.address(),
            &programs.escrow,
            &params,
            asset_id,
            funds_asset_id,
            app_id,
        )
        .await?;
        let signed_fund_min_balance = creator.sign_transaction(&to_sign.fund_min_balance_tx)?;
        let tx_id = submit_setup_capi_escrow(
            &algod,
            &SetupCentralEscrowSigned {
                optin_to_capi_asset_tx: to_sign.optin_to_capi_asset_tx,
                optin_to_funds_asset_tx: to_sign.optin_to_funds_asset_tx,
                fund_min_balance_tx: signed_fund_min_balance,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &tx_id).await?;

        log::debug!(
            "Created Capi asset, id: {asset_id}, app id: {app_id}, escrow: {}",
            to_sign.escrow.address()
        );

        Ok(CapiAssetFlowRes {
            asset_id,
            app_id,
            escrow: to_sign.escrow,
        })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CapiAssetFlowRes {
        pub asset_id: CapiAssetId,
        pub app_id: CapiAppId,
        pub escrow: ContractAccount,
    }
}
