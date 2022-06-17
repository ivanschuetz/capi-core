#[cfg(test)]
pub use test::create_dao_flow;
#[cfg(test)]
pub mod test {
    use crate::flows::create_dao::setup_dao::{Escrows, Programs};
    use crate::flows::create_dao::{
        model::{Dao, SetupDaoSigned},
        setup::create_shares::{create_assets, submit_create_assets, CrateDaoAssetsSigned},
        setup_dao::{setup_dao_txs, submit_setup_dao},
    };
    use crate::testing::network_test_util::TestDeps;
    use anyhow::Result;
    use mbase::api::version::{Version, VersionedTealSourceTemplate};
    use mbase::teal::load_teal_template;

    pub async fn create_dao_flow(td: &TestDeps) -> Result<Dao> {
        let algod = &td.algod;

        // Create asset first: id needed in app template
        let create_assets_txs = create_assets(
            &algod,
            &td.creator.address(),
            &td.specs,
            &td.programs.central_app_approval,
            &td.programs.central_app_clear,
            td.precision,
            &td.dao_deps(),
        )
        .await?;

        let signed_create_shares_tx = td
            .creator
            .sign_transaction(create_assets_txs.create_shares_tx)?;
        let signed_create_app_tx = td
            .creator
            .sign_transaction(create_assets_txs.create_app_tx)?;

        let create_assets_res = submit_create_assets(
            algod,
            &CrateDaoAssetsSigned {
                create_shares: signed_create_shares_tx,
                create_app: signed_create_app_tx,
            },
        )
        .await?;

        // Rest of create dao txs
        let to_sign = setup_dao_txs(
            algod,
            &td.specs,
            td.creator.address(),
            create_assets_res.shares_asset_id,
            td.funds_asset_id,
            &td.programs,
            td.precision,
            create_assets_res.app_id,
            &td.dao_deps(),
        )
        .await?;

        let fund_customer_escrow_tx = td
            .creator
            .sign_transaction(to_sign.customer_escrow_funding_tx)?;
        let signed_fund_app_tx = td.creator.sign_transaction(to_sign.fund_app_tx)?;
        let signed_setup_app_tx = td.creator.sign_transaction(to_sign.setup_app_tx)?;
        let signed_transfer_shares_to_app_tx = td
            .creator
            .sign_transaction(to_sign.transfer_shares_to_app_tx)?;

        // Create the asset (submit signed tx) and generate escrow funding tx
        // Note that the escrow is generated after the asset, because it uses the asset id (in teal, inserted with template)

        let create_res = submit_setup_dao(
            &algod,
            SetupDaoSigned {
                specs: to_sign.specs,
                creator: td.creator.address(),
                shares_asset_id: create_assets_res.shares_asset_id,
                funds_asset_id: td.funds_asset_id.clone(),
                fund_customer_escrow_tx,
                customer_escrow_optin_to_funds_asset_tx: to_sign
                    .customer_escrow_optin_to_funds_asset_tx,
                app_funding_tx: signed_fund_app_tx,
                setup_app_tx: signed_setup_app_tx,
                customer_escrow: to_sign.customer_escrow,
                app_id: create_assets_res.app_id,
                transfer_shares_to_app_tx: signed_transfer_shares_to_app_tx,
            },
        )
        .await?;

        log::debug!("Created dao: {:?}", create_res.dao);

        Ok(create_res.dao)
    }

    pub fn test_programs() -> Result<Programs> {
        Ok(Programs {
            central_app_approval: VersionedTealSourceTemplate::new(
                load_teal_template("dao_app_approval")?,
                Version(1),
            ),
            central_app_clear: VersionedTealSourceTemplate::new(
                load_teal_template("dao_app_clear")?,
                Version(1),
            ),
            escrows: Escrows {
                customer_escrow: VersionedTealSourceTemplate::new(
                    load_teal_template("customer_escrow")?,
                    Version(1),
                ),
            },
        })
    }
}
