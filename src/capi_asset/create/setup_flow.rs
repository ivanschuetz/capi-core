#[cfg(test)]
pub mod test_flow {
    use crate::algo_helpers::send_tx_and_wait;
    use crate::api::api::Api;
    use crate::api::contract::Contract;
    use crate::api::version::Version;
    use crate::capi_asset::capi_app_id::CapiAppId;
    use crate::capi_asset::capi_asset_id::{CapiAssetAmount, CapiAssetId};
    use crate::capi_asset::create::create_capi_app::create_app;
    use crate::capi_asset::create::create_capi_asset::create_capi_asset;
    use crate::capi_asset::create::setup_capi_escrow::{
        setup_capi_escrow, submit_setup_capi_escrow, SetupCentralEscrowSigned, MIN_BALANCE,
    };
    use crate::funds::FundsAssetId;
    use crate::network_util::wait_for_pending_transaction;
    use crate::testing::TESTS_DEFAULT_PRECISION;
    use algonaut::algod::v2::Algod;
    use algonaut::core::{Address, SuggestedTransactionParams};
    use algonaut::transaction::account::Account;
    use algonaut::transaction::{Pay, TxnBuilder};
    use anyhow::Result;

    /// creates capi asset and app and setups app
    pub async fn setup_capi_asset_flow(
        algod: &Algod,
        api: &dyn Api,
        creator: &Account,
        capi_supply: CapiAssetAmount,
        funds_asset_id: FundsAssetId,
    ) -> Result<CapiAssetFlowRes> {
        // create asset
        let params = algod.suggested_transaction_params().await?;

        let to_sign = create_capi_asset(capi_supply, &creator.address(), &params).await?;
        let signed = creator.sign_transaction(to_sign.create_capi_asset_tx)?;
        log::debug!("Will submit crate capi asset..");
        let p_tx = send_tx_and_wait(algod, &signed).await?;
        let asset_id_opt = p_tx.asset_index;
        assert!(asset_id_opt.is_some());
        let asset_id = CapiAssetId(asset_id_opt.unwrap());

        // create app

        let app_approval_template = api.template(Contract::CapiAppApproval, Version(1))?;
        let app_clear_template = api.template(Contract::CapiAppClear, Version(1))?;

        let to_sign_app = create_app(
            &algod,
            &app_approval_template,
            &app_clear_template,
            &creator.address(),
            capi_supply,
            TESTS_DEFAULT_PRECISION,
            &params,
            asset_id,
            funds_asset_id,
        )
        .await?;
        let signed = creator.sign_transaction(to_sign_app)?;
        log::debug!("Will submit crate capi app..");
        // crate::teal::debug_teal_rendered(&[signed.clone()], "capi_app_approval").unwrap();
        let p_tx = send_tx_and_wait(algod, &signed).await?;
        let app_id_opt = p_tx.application_index;
        assert!(app_id_opt.is_some());
        let app_id = CapiAppId(app_id_opt.unwrap());

        log::debug!(
            "Created Capi asset, id: {asset_id}, app id: {app_id}, app address: {}",
            app_id.address()
        );

        // setup app

        // send payment for min balance (needed for optins)
        // note that here we don't do it in the same step / group as the app setup, since it complicates the TEAL
        // and we don't need performance/UX here, as this is executed only once in Capi's lifetime (by owner)
        let min_balance_payment = TxnBuilder::with(
            &params,
            Pay::new(creator.address(), app_id.address(), MIN_BALANCE).build(),
        )
        .build()?;
        let signed_min_balance_payment = creator.sign_transaction(min_balance_payment)?;
        send_tx_and_wait(algod, &signed_min_balance_payment).await?;

        setup_capi_app(&algod, &params, &creator, funds_asset_id, asset_id, app_id).await?;

        Ok(CapiAssetFlowRes {
            asset_id,
            app_id,
            supply: capi_supply,
            owner_mnemonic: creator.mnemonic(),
        })
    }

    /// Funds (min balance), and opts-in the app's escrow to assets (capi and funds asset)
    pub async fn setup_capi_app(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        funder: &Account,
        funds_asset_id: FundsAssetId,
        capi_asset_id: CapiAssetId,
        capi_app_id: CapiAppId,
    ) -> Result<()> {
        let to_sign = setup_capi_escrow(
            &funder.address(),
            &params,
            capi_asset_id,
            funds_asset_id,
            capi_app_id,
        )
        .await?;
        let signed_app_call = funder.sign_transaction(to_sign.app_call_tx)?;
        let tx_id = submit_setup_capi_escrow(
            &algod,
            &SetupCentralEscrowSigned {
                app_call_tx: signed_app_call,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &tx_id).await?;

        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CapiAssetFlowRes {
        pub asset_id: CapiAssetId,
        pub app_id: CapiAppId,
        pub supply: CapiAssetAmount,
        pub owner_mnemonic: String, // for now a string, since Account isn't clonable
    }

    impl CapiAssetFlowRes {
        pub fn app_address(&self) -> Address {
            self.app_id.address()
        }
    }
}
