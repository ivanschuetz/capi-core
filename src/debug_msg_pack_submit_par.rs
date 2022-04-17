use std::fs;

use serde::Serialize;

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use std::{convert::TryInto, str::FromStr};

    use algonaut::core::Address;
    use anyhow::{Error, Result};
    use rust_decimal::Decimal;
    use tokio::test;

    use crate::{
        capi_asset::{
            capi_app_id::CapiAppId, capi_asset_dao_specs::CapiAssetDaoDeps,
            capi_asset_id::CapiAssetId,
        },
        decimal_util::AsDecimal,
        dependencies,
        flows::{
            claim::claim::{submit_claim, ClaimSigned},
            create_dao::{
                setup::{
                    create_app::render_central_app_approval_v1,
                    customer_escrow::{
                        render_and_compile_customer_escrow, render_customer_escrow_v1,
                    },
                    investing_escrow::render_investing_escrow_v1,
                    setup_app,
                },
                share_amount::ShareAmount,
                shares_percentage::SharesPercentage,
                storage::load_dao::DaoAppId,
            },
            drain::drain::{submit_drain_customer_escrow, DrainCustomerEscrowSigned},
            invest::{invest::submit_invest, model::InvestSigned},
            withdraw::withdraw::{submit_withdraw, WithdrawSigned},
        },
        funds::{FundsAmount, FundsAssetId},
        teal::load_teal_template,
        testing::{test_data::creator, TESTS_DEFAULT_PRECISION},
    };

    // helper for environments that don't allow to open directly the TEAL debugger (e.g. WASM)
    // Copy the parameters, serialized to msg pack, here and run the test
    // (Note that Algonaut doesn't suppot JSON deserialization yet, otherwise we could use it alternatively)
    #[test]
    #[ignore]
    async fn debug_msg_pack_submit_par() -> Result<()> {
        let algod = dependencies::algod_for_tests();

        // Set parameters to match current environment

        let shares_asset_id = 20;
        let shares_price = FundsAmount::new(10000000);
        let funds_asset_id = FundsAssetId(6);
        let share_supply = ShareAmount::new(100);
        let investors_share = ShareAmount::new(40);
        let app_id = DaoAppId(123);
        let capi_app_id = CapiAppId(123);
        let capi_share = 123u64.as_decimal().try_into()?;
        let owner = creator();

        let locking_escrow: Address = "XAU2GR4AJTOAESPTO77NIKC72TTIXDNCIP3LI67PFRCWQTN35JD26ENO74"
            .parse()
            .map_err(Error::msg)?;
        let capi_escrow_address: Address =
            "AAU2GR4AJTOAESPTO77NIKC72TTIXDNCIP3LI67PFRCWQTN35JD26ENO75"
                .parse()
                .map_err(Error::msg)?;
        // let capi_deps = &CapiAssetDaoDeps {
        //     escrow: capi_escrow_address,
        //     escrow_percentage: Decimal::from_str("0.1").unwrap().try_into()?,
        //     app_id: CapiAppId(123),
        //     asset_id: CapiAssetId(123),
        // };

        // update rendered teal if needed - since teal was rendered with WASM,
        // it's possible that the saved teal used here is outdated

        let approval_template = load_teal_template("dao_app_approval")?;
        render_central_app_approval_v1(
            &approval_template,
            &owner.address(),
            share_supply,
            TESTS_DEFAULT_PRECISION,
            investors_share,
            &capi_escrow_address,
            capi_app_id,
            capi_share,
            shares_price,
        )?;

        let investing_escrow_template = load_teal_template("investing_escrow")?;
        render_investing_escrow_v1(
            &investing_escrow_template,
            shares_asset_id,
            &shares_price,
            &funds_asset_id,
            &locking_escrow,
            app_id,
        )?;

        // insert msg pack serialized bytes
        let bytes = vec![];

        // let signed: ClaimSigned = rmp_serde::from_slice(&bytes).unwrap();

        // let signed: WithdrawSigned = rmp_serde::from_slice(&bytes).unwrap();
        // submit_withdraw(&algod, &signed).await?;

        // let signed: DrainCustomerEscrowSigned = rmp_serde::from_slice(&bytes).unwrap();
        // submit_drain_customer_escrow(&algod, &signed).await?;

        let signed: InvestSigned = rmp_serde::from_slice(&bytes).unwrap();
        submit_invest(&algod, &signed).await?;

        Ok(())
    }
}

#[allow(dead_code)]
pub fn log_to_msg_pack<T>(obj: &T)
where
    T: Serialize + ?Sized,
{
    log::info!("log_to_msg_pack:");
    // Unwrap: only for debugging
    log::info!("{:?}", rmp_serde::to_vec_named(obj).unwrap());
}

/// To import things in SDKs that use signed bytes
#[allow(dead_code)]
pub fn log_to_msg_pack_signed<T>(obj: &T)
where
    T: Serialize + ?Sized,
{
    log::info!("log_to_msg_pack (signed):");
    // Unwrap: only for debugging
    let bytes = rmp_serde::to_vec_named(obj).unwrap();
    let signed_bytes = bytes.into_iter().map(|b| b as i8).collect::<Vec<i8>>();
    log::info!("{:?}", signed_bytes);
}

#[allow(dead_code)]
pub fn write_bytes_to_tmp_file(bytes: &[u8]) {
    // just a (gitignore) file in the root directory
    fs::write(&format!("./some_bytes"), bytes).unwrap();
}
