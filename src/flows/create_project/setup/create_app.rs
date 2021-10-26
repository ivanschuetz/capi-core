use algonaut::{
    algod::v2::Algod,
    core::Address,
    transaction::{transaction::StateSchema, CreateApplication, Transaction, TxnBuilder},
};
use anyhow::{anyhow, Result};
use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::{
    decimal_util::AsDecimal,
    teal::{render_template, TealSource, TealSourceTemplate},
};

pub async fn create_app_tx(
    algod: &Algod,
    approval_source: TealSourceTemplate,
    clear_source: TealSource,
    creator: &Address,
    asset_id: u64,
    asset_supply: u64,
    precision: u64,
    investors_share: u64,
) -> Result<Transaction> {
    log::debug!(
        "Creating central app with asset id: {}, supply: {}",
        asset_id,
        asset_supply
    );
    let approval_source = render_central_app(
        approval_source,
        asset_id,
        asset_supply,
        precision,
        investors_share,
    )?;

    let compiled_approval_program = algod.compile_teal(&approval_source.0).await?;
    let compiled_clear_program = algod.compile_teal(&clear_source.0).await?;

    let params = algod.suggested_transaction_params().await?;
    let tx = TxnBuilder::with(
        params,
        CreateApplication::new(
            *creator,
            compiled_approval_program.clone().program,
            compiled_clear_program.program,
            StateSchema {
                number_ints: 1, // "total received"
                number_byteslices: 0,
            },
            StateSchema {
                number_ints: 2, // for investors: "shares" and "already retrieved"
                number_byteslices: 0,
            },
        )
        .build(),
    )
    .build();

    Ok(tx)
}

fn render_central_app(
    source: TealSourceTemplate,
    asset_id: u64,
    asset_supply: u64,
    precision: u64,
    investors_share: u64,
) -> Result<TealSource> {
    let precision_square = precision
        .checked_pow(2)
        .ok_or(anyhow!("Precision squared overflow: {}", precision))?;
    let investors_share =
        ((investors_share.as_decimal() / 100.as_decimal()) * precision.as_decimal()).floor();

    let source = render_template(
        source,
        RenderCentralAppContext {
            asset_id: asset_id.to_string(),
            asset_supply: asset_supply.to_string(),
            investors_share: investors_share.to_string(),
            precision: precision.to_string(),
            precision_square: precision_square.to_string(),
        },
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("app_central_approval", source.clone())?; // debugging
    Ok(source)
}

#[derive(Serialize)]
struct RenderCentralAppContext {
    asset_id: String,
    asset_supply: String,
    investors_share: String,
    precision: String,
    precision_square: String,
}

#[cfg(test)]
mod tests {
    use crate::{
        dependencies,
        network_util::wait_for_pending_transaction,
        teal::{load_teal, load_teal_template},
        testing::{network_test_util::reset_network, test_data::creator, TESTS_DEFAULT_PRECISION},
    };
    use algonaut::{
        model::algod::v2::TealKeyValue,
        transaction::{transaction::StateSchema, Transaction, TransactionType},
    };
    use anyhow::{anyhow, Result};
    use serial_test::serial;
    use tokio::test;

    use super::create_app_tx;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_create_app() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();

        let approval_template = load_teal_template("app_central_approval")?;
        let clear_source = load_teal("app_central_clear")?;

        // asset id and supply aren't used here so we can pass anything (0 in this case)
        let tx = create_app_tx(
            &algod,
            approval_template,
            clear_source,
            &creator.address(),
            0,
            0,
            TESTS_DEFAULT_PRECISION,
            40,
        )
        .await?;

        let signed_tx = creator.sign_transaction(&tx)?;
        let res = algod.broadcast_signed_transaction(&signed_tx).await?;

        println!("App created! tx id: {:?}", res.tx_id);
        let p_tx_opt = wait_for_pending_transaction(&algod, &res.tx_id).await?;
        assert!(p_tx_opt.is_some());
        let p_tx = p_tx_opt.unwrap();
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
            global_state_schema(&tx)?.unwrap().number_ints,
            params_global_schema.num_uint
        );
        assert_eq!(
            global_state_schema(&tx)?.unwrap().number_byteslices,
            params_global_schema.num_byte_slice
        );
        assert_eq!(
            local_state_schema(&tx)?.unwrap().number_ints,
            params_local_schema.num_uint
        );
        assert_eq!(
            local_state_schema(&tx)?.unwrap().number_byteslices,
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
