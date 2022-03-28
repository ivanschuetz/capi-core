#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::{
    api::version::VersionedTealSourceTemplate,
    capi_asset::capi_asset_id::{CapiAssetAmount, CapiAssetId},
    funds::FundsAssetId,
    teal::{render_template_new, TealSource, TealSourceTemplate},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, CompiledTeal, SuggestedTransactionParams},
    transaction::{transaction::StateSchema, CreateApplication, Transaction, TxnBuilder},
};
use anyhow::{anyhow, Result};

/// Capi app: remembers total dividend retrieved (global) and already retrieved dividend (local), to prevent double claiming.
#[allow(clippy::too_many_arguments)]
pub async fn create_app(
    algod: &Algod,
    approval_template: &VersionedTealSourceTemplate,
    clear_template: &VersionedTealSourceTemplate,
    sender: &Address,
    asset_supply: CapiAssetAmount,
    precision: u64,
    params: &SuggestedTransactionParams,
    asset_id: CapiAssetId,
    funds_asset_id: FundsAssetId,
    capi_owner: &Address,
) -> Result<Transaction> {
    log::debug!("Creating capi app");

    let compiled_approval_program = render_and_compile_app_approval(
        algod,
        approval_template,
        asset_supply,
        precision,
        asset_id,
        funds_asset_id,
        capi_owner,
    )
    .await?;
    let compiled_clear_program = render_and_compile_app_clear(algod, clear_template).await?;

    let tx = TxnBuilder::with(
        params,
        CreateApplication::new(
            *sender,
            compiled_approval_program.clone(),
            compiled_clear_program,
            StateSchema {
                number_ints: 1, // "total received"
                number_byteslices: 0,
            },
            StateSchema {
                number_ints: 3, // for investors: "shares", "already retrieved"
                number_byteslices: 0,
            },
        )
        .build(),
    )
    .build()?;

    Ok(tx)
}

pub async fn render_and_compile_app_approval(
    algod: &Algod,
    template: &VersionedTealSourceTemplate,
    asset_supply: CapiAssetAmount,
    precision: u64,
    asset_id: CapiAssetId,
    funds_asset_id: FundsAssetId,
    capi_owner: &Address,
) -> Result<CompiledTeal> {
    let source = match template.version.0 {
        1 => render_app_v1(
            &template.template,
            asset_supply,
            precision,
            asset_id,
            funds_asset_id,
            capi_owner,
        ),
        _ => Err(anyhow!(
            "Dao app approval version not supported: {:?}",
            template.version
        )),
    }?;

    Ok(algod.compile_teal(&source.0).await?)
}

pub fn render_app_v1(
    source: &TealSourceTemplate,
    asset_supply: CapiAssetAmount,
    precision: u64,
    asset_id: CapiAssetId,
    funds_asset_id: FundsAssetId,
    capi_owner: &Address,
) -> Result<TealSource> {
    let source = render_template_new(
        source,
        &[
            ("TMPL_CAPI_ASSET_ID", &asset_id.0.to_string()),
            ("TMPL_FUNDS_ASSET_ID", &funds_asset_id.0.to_string()),
            ("TMPL_SHARE_SUPPLY", &asset_supply.0.to_string()),
            ("TMPL_PRECISION", &precision.to_string()),
            ("TMPL_CAPI_OWNER", &capi_owner.to_string()),
        ],
    )?;

    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("capi_app_approval", source.clone())?; // debugging
    Ok(source)
}

pub async fn render_and_compile_app_clear(
    algod: &Algod,
    template: &VersionedTealSourceTemplate,
) -> Result<CompiledTeal> {
    let source = match template.version.0 {
        1 => render_central_app_clear_v1(&template.template),
        _ => Err(anyhow!(
            "Dao app clear version not supported: {:?}",
            template.version
        )),
    }?;

    Ok(algod.compile_teal(&source.0).await?)
}

pub fn render_central_app_clear_v1(template: &TealSourceTemplate) -> Result<TealSource> {
    Ok(TealSource(template.0.clone()))
}

#[cfg(test)]
mod tests {
    use crate::{
        algo_helpers::send_tx_and_wait,
        api::version::{Version, VersionedTealSourceTemplate},
        capi_asset::{
            capi_asset_id::{CapiAssetAmount, CapiAssetId},
            create::create_capi_app::create_app,
        },
        dependencies,
        funds::FundsAssetId,
        teal::load_teal_template,
        testing::{network_test_util::test_init, test_data::creator, TESTS_DEFAULT_PRECISION},
    };
    use algonaut::{
        model::algod::v2::TealKeyValue,
        transaction::{transaction::StateSchema, Transaction, TransactionType},
    };
    use anyhow::{anyhow, Result};
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_create_app() -> Result<()> {
        test_init()?;

        // deps
        let algod = dependencies::algod_for_tests();
        let creator = creator();

        let approval_template =
            VersionedTealSourceTemplate::new(load_teal_template("capi_app_approval")?, Version(1));
        let clear_template =
            VersionedTealSourceTemplate::new(load_teal_template("capi_app_clear")?, Version(1));

        let params = algod.suggested_transaction_params().await?;

        // asset supply, capi and funds asset id aren't used here so we can pass anything (0 in this case)
        let tx = create_app(
            &algod,
            &approval_template,
            &clear_template,
            &creator.address(),
            CapiAssetAmount::new(1),
            TESTS_DEFAULT_PRECISION,
            &params,
            CapiAssetId(0),
            FundsAssetId(0),
            &creator.address(),
        )
        .await?;

        let signed_tx = creator.sign_transaction(tx)?;
        let p_tx = send_tx_and_wait(&algod, &signed_tx).await?;

        assert!(p_tx.application_index.is_some());
        let p_tx_app_index = p_tx.application_index.unwrap();

        let creator_infos = algod.account_information(&creator.address()).await?;

        let apps = creator_infos.created_apps;
        assert_eq!(1, apps.len());

        let app = &apps[0];
        assert!(!app.params.approval_program.is_empty());
        assert!(!app.params.clear_state_program.is_empty());
        assert_eq!(creator.address(), app.params.creator);
        assert_eq!(Vec::<TealKeyValue>::new(), app.params.global_state);
        assert_eq!(p_tx_app_index, app.id); // just a general sanity check: id returning in pending tx is the same as in creator account
        assert!(app.params.global_state_schema.is_some());
        assert!(app.params.local_state_schema.is_some());

        // the repetition here wouldn't be needed if algonaut used the same types for transaction and algod::v2..
        let params_global_schema = app.params.global_state_schema.as_ref().unwrap();
        let params_local_schema = app.params.local_state_schema.as_ref().unwrap();
        assert_eq!(
            global_state_schema(&signed_tx.transaction)?
                .unwrap()
                .number_ints,
            params_global_schema.num_uint
        );
        assert_eq!(
            global_state_schema(&signed_tx.transaction)?
                .unwrap()
                .number_byteslices,
            params_global_schema.num_byte_slice
        );
        assert_eq!(
            local_state_schema(&signed_tx.transaction)?
                .unwrap()
                .number_ints,
            params_local_schema.num_uint
        );
        assert_eq!(
            local_state_schema(&signed_tx.transaction)?
                .unwrap()
                .number_byteslices,
            params_local_schema.num_byte_slice
        );
        Ok(())
    }

    fn global_state_schema(tx: &Transaction) -> Result<Option<StateSchema>> {
        match &tx.txn_type {
            TransactionType::ApplicationCallTransaction(c) => Ok(c.global_state_schema.to_owned()),
            _ => Err(anyhow!(
                "Invalid state: tx is expected to be an app call tx: {:?}",
                tx
            )),
        }
    }

    fn local_state_schema(tx: &Transaction) -> Result<Option<StateSchema>> {
        match &tx.txn_type {
            TransactionType::ApplicationCallTransaction(c) => Ok(c.local_state_schema.to_owned()),
            _ => Err(anyhow!(
                "Invalid state: tx is expected to be an app call tx: {:?}",
                tx
            )),
        }
    }
}
