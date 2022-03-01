use serde::Serialize;

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use anyhow::{Error, Result};
    use tokio::test;

    use crate::{
        dependencies,
        flows::{
            create_project::{
                setup::{
                    create_app::render_central_app,
                    customer_escrow::{render_and_compile_customer_escrow, render_customer_escrow},
                    investing_escrow::render_investing_escrow,
                },
                share_amount::ShareAmount,
            },
            drain::drain::{submit_drain_customer_escrow, DrainCustomerEscrowSigned},
            harvest::harvest::{submit_harvest, HarvestSigned},
            invest::{invest::submit_invest, model::InvestSigned},
            withdraw::withdraw::{submit_withdraw, WithdrawSigned},
        },
        funds::{FundsAmount, FundsAssetId},
        teal::load_teal_template,
        testing::TESTS_DEFAULT_PRECISION,
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

        let central_escrow = "J7RHJEAARYDZZ6QUKH4KKICZK64PS4UTJPVLEI3WN5SNU47GHWD4PTOOIQ"
            .parse()
            .map_err(Error::msg)?;
        let customer_escrow = "MHQSDG3IAGGRQWNNHMXDMAY6K54UAOXGFJUTWNHXK5C4FVC7AGWK66KQPQ"
            .parse()
            .map_err(Error::msg)?;
        let locking_escrow = "XAU2GR4AJTOAESPTO77NIKC72TTIXDNCIP3LI67PFRCWQTN35JD26ENO74"
            .parse()
            .map_err(Error::msg)?;

        // update rendered teal if needed - since teal was rendered with WASM,
        // it's possible that the saved teal used here is outdated

        let approval_template = load_teal_template("app_central_approval")?;
        render_central_app(
            &approval_template,
            share_supply,
            TESTS_DEFAULT_PRECISION,
            investors_share,
            &customer_escrow,
            &central_escrow,
        )?;

        let customer_escrow_template = load_teal_template("customer_escrow")?;
        render_customer_escrow(&central_escrow, &customer_escrow_template)?;

        let investing_escrow_template = load_teal_template("investing_escrow")?;
        render_investing_escrow(
            &investing_escrow_template,
            shares_asset_id,
            &shares_price,
            &funds_asset_id,
            &locking_escrow,
        )?;

        // insert msg pack serialized bytes
        let bytes = vec![];

        // let signed: HarvestSigned = rmp_serde::from_slice(&bytes).unwrap();

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
