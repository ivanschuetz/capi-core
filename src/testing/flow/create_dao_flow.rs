#[cfg(test)]
pub use test::create_dao_flow;
#[cfg(test)]
pub mod test {
    use crate::api::version::VersionedTealSourceTemplate;
    use crate::flows::create_dao::create_dao::{Escrows, Programs};
    use crate::teal::load_teal_template;
    use crate::testing::network_test_util::TestDeps;
    use crate::{
        api::version::Version,
        flows::create_dao::{
            create_dao::{create_dao_txs, submit_create_dao},
            model::{CreateDaoSigned, Dao},
            setup::create_shares::{create_assets, submit_create_assets, CrateDaoAssetsSigned},
        },
    };
    use algonaut::core::Address;
    use anyhow::Result;

    pub async fn create_dao_flow(td: &TestDeps) -> Result<Dao> {
        create_dao_flow_with_owner(td, &td.creator.address()).await
    }

    pub async fn create_dao_flow_with_owner(td: &TestDeps, owner: &Address) -> Result<Dao> {
        let algod = &td.algod;

        // Create asset first: id needed in app template
        let create_assets_txs = create_assets(
            &algod,
            &td.creator.address(),
            owner,
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
        let to_sign = create_dao_txs(
            algod,
            &td.specs,
            td.creator.address(),
            *owner,
            create_assets_res.shares_asset_id,
            td.funds_asset_id,
            &td.programs,
            td.precision,
            create_assets_res.app_id,
            &td.dao_deps(),
        )
        .await?;

        let mut signed_funding_txs = vec![];
        for tx in to_sign.escrow_funding_txs {
            signed_funding_txs.push(td.creator.sign_transaction(tx)?);
        }
        let signed_fund_app_tx = td.creator.sign_transaction(to_sign.fund_app_tx)?;
        let signed_setup_app_tx = td.creator.sign_transaction(to_sign.setup_app_tx)?;

        let signed_xfer_shares_to_invest_escrow = td
            .creator
            .sign_transaction(to_sign.xfer_shares_to_invest_escrow)?;

        // Create the asset (submit signed tx) and generate escrow funding tx
        // Note that the escrow is generated after the asset, because it uses the asset id (in teal, inserted with template)

        let create_res = submit_create_dao(
            &algod,
            CreateDaoSigned {
                specs: to_sign.specs,
                creator: td.creator.address(),
                shares_asset_id: create_assets_res.shares_asset_id,
                funds_asset_id: td.funds_asset_id.clone(),
                escrow_funding_txs: signed_funding_txs,
                optin_txs: to_sign.optin_txs,
                app_funding_tx: signed_fund_app_tx,
                setup_app_tx: signed_setup_app_tx,
                xfer_shares_to_invest_escrow: signed_xfer_shares_to_invest_escrow,
                invest_escrow: to_sign.invest_escrow,
                customer_escrow: to_sign.customer_escrow,
                app_id: create_assets_res.app_id,
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
                invest_escrow: VersionedTealSourceTemplate::new(
                    load_teal_template("investing_escrow")?,
                    Version(1),
                ),
            },
        })
    }
}
