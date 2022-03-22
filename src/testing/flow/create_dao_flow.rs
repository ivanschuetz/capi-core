#[cfg(test)]
pub use test::{capi_programs, create_dao_flow, programs};

#[cfg(test)]
pub mod test {
    use crate::flows::create_dao::{
        create_dao::{create_dao_txs, submit_create_dao, CapiPrograms},
        model::{CreateDaoSigned, Dao},
        setup::create_shares::{create_assets, submit_create_assets, CrateDaoAssetsSigned},
    };
    use crate::{
        flows::create_dao::create_dao::{Escrows, Programs},
        teal::{load_teal, load_teal_template},
        testing::network_test_util::TestDeps,
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
            // in the default test flows, the creator is always the owner
            &td.creator.address(),
            &td.specs,
            &td.programs,
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
                setup_app_tx: signed_setup_app_tx,
                xfer_shares_to_invest_escrow: signed_xfer_shares_to_invest_escrow,
                invest_escrow: to_sign.invest_escrow,
                locking_escrow: to_sign.locking_escrow,
                central_escrow: to_sign.central_escrow,
                customer_escrow: to_sign.customer_escrow,
                app_id: create_assets_res.app_id,
            },
        )
        .await?;

        log::debug!("Created dao: {:?}", create_res.dao);

        Ok(create_res.dao)
    }

    pub fn programs() -> Result<Programs> {
        Ok(Programs {
            central_app_approval: load_teal_template("app_central_approval")?,
            central_app_clear: load_teal("app_central_clear")?,
            escrows: Escrows {
                central_escrow: load_teal_template("central_escrow")?,
                customer_escrow: load_teal_template("customer_escrow")?,
                invest_escrow: load_teal_template("investing_escrow")?,
                locking_escrow: load_teal_template("locking_escrow")?,
            },
        })
    }

    pub fn capi_programs() -> Result<CapiPrograms> {
        Ok(CapiPrograms {
            app_approval: load_teal_template("app_capi_approval")?,
            app_clear: load_teal("app_capi_clear")?,
            escrow: load_teal_template("capi_escrow")?,
        })
    }
}
