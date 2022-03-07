#[cfg(test)]
pub use test::{harvest_capi_flow, harvest_capi_precs};

#[cfg(test)]
mod test {
    use crate::{
        capi_asset::{
            capi_app_id::CapiAppId,
            capi_asset_dao_specs::CapiAssetDaoDeps,
            capi_asset_id::CapiAssetAmount,
            create::test_flow::test_flow::CapiAssetFlowRes,
            harvest::harvest::{harvest, submit_harvest, HarvestSigned},
        },
        funds::{FundsAmount, FundsAssetId},
        network_util::wait_for_pending_transaction,
        testing::{
            create_and_submit_txs::{
                optin_to_app_submit, optin_to_asset_submit, transfer_tokens_and_pay_fee_submit,
            },
            flow::{
                create_project_flow::create_project_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
                lock_capi_asset_flow::lock_capi_asset_flow,
            },
            test_data::{creator, customer, investor2, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
    };
    use algonaut::{
        algod::v2::Algod,
        transaction::{account::Account, contract_account::ContractAccount},
    };
    use anyhow::Result;
    use rust_decimal::{prelude::ToPrimitive, Decimal};
    use std::{convert::TryInto, str::FromStr};

    pub async fn harvest_capi_flow(
        algod: &Algod,
        investor: &Account,
        amount: FundsAmount,
        funds_asset_id: FundsAssetId,
        app_id: CapiAppId,
        capi_escrow: &ContractAccount,
    ) -> Result<()> {
        let to_sign = harvest(
            &algod,
            &investor.address(),
            app_id,
            funds_asset_id,
            amount,
            capi_escrow,
        )
        .await?;
        let signed_app_call_tx = investor.sign_transaction(&to_sign.app_call_tx)?;

        let submit_lock_tx_id = submit_harvest(
            &algod,
            &HarvestSigned {
                app_call_tx_signed: signed_app_call_tx,
                harvest_tx: to_sign.harvest_tx,
            },
        )
        .await?;
        wait_for_pending_transaction(&algod, &submit_lock_tx_id).await?;

        Ok(())
    }

    #[cfg(test)]
    pub async fn harvest_capi_precs(
        algod: &Algod,
        capi_creator: &Account,
        funds_asset_id: FundsAssetId,
        harvester: &Account,
        asset_amount: CapiAssetAmount,
        // Fee sent to the capi escrow after the investor locks their shares. This is the amount we harvest from.
        send_to_escrow_after_investor_locked: FundsAmount,
        capi_deps: &CapiAssetFlowRes,
    ) -> Result<()> {
        let params = algod.suggested_transaction_params().await?;

        // opt ins

        optin_to_asset_submit(&algod, &harvester, capi_deps.asset_id.0).await?;
        optin_to_app_submit(&algod, &params, &harvester, capi_deps.app_id.0).await?;

        // send capi assets to investor

        transfer_tokens_and_pay_fee_submit(
            &algod,
            &params,
            &capi_creator,
            &capi_creator,
            &harvester.address(),
            capi_deps.asset_id.0,
            asset_amount.val(),
        )
        .await?;

        // lock capi assets

        lock_capi_asset_flow(
            &algod,
            &harvester,
            asset_amount,
            capi_deps.asset_id,
            capi_deps.app_id,
            &capi_deps.escrow.address(),
        )
        .await?;

        // These can be created locally, as the DAO flow is contained here and irrelevant for the capi token testing.
        // We'll assume for now that investor2 isn't used outside.
        let drainer = investor2();
        let customer = customer();

        let capi_dao_deps = CapiAssetDaoDeps {
            escrow: *capi_deps.escrow.address(),
            // value here has to ensure that we always get an integer result when diving an integer by it
            escrow_percentage: Decimal::from_str("0.1").unwrap().try_into().unwrap(),
            app_id: capi_deps.app_id,
            asset_id: capi_deps.asset_id,
        };

        let project = create_project_flow(
            &algod,
            &creator(),
            &project_specs(),
            funds_asset_id,
            TESTS_DEFAULT_PRECISION,
            &capi_dao_deps,
        )
        .await?;

        // calculate a to-be-drained amount, such that we get exactly the expected funds amount in the capi escrow
        let central_funds_decimal = send_to_escrow_after_investor_locked.as_decimal()
            / capi_dao_deps.escrow_percentage.value();
        // unwrap: we ensured with parameters above that central_funds_decimal is an integer
        let central_funds = FundsAmount::new(central_funds_decimal.to_u64().unwrap());
        log::debug!("Harvest precs: Will pay and drain funds: {central_funds}");

        // let central_funds = FundsAmount(10 * 1_000_000);

        customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            funds_asset_id,
            central_funds,
            &project.project,
            &capi_dao_deps,
        )
        .await?;

        Ok(())
    }
}
