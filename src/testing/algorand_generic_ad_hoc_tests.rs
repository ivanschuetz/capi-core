/// Convenience to write ad hoc tests
/// Ad hoc meaning something we want to test quickly and not necessarily upload

#[cfg(test)]
use super::test_data::creator;
#[cfg(test)]
use crate::{dependencies::algod, network_util::wait_for_pending_transaction, teal::load_teal};
#[cfg(test)]
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::Pay,
    transaction::{
        transaction::StateSchema, tx_group::TxGroup, CreateApplication, Transaction, TxnBuilder,
    },
};
#[cfg(test)]
use anyhow::Result;
#[cfg(test)]
use tokio::test;

#[allow(dead_code)]
#[cfg(test)]
pub async fn create_always_approves_app(algod: &Algod, sender: &Address) -> Result<Transaction> {
    let always_succeeds_source = load_teal("always_succeeds").unwrap();

    let compiled_approval_program = algod.compile_teal(&always_succeeds_source.0).await?;
    let compiled_clear_program = algod.compile_teal(&always_succeeds_source.0).await?;

    let params = algod.suggested_transaction_params().await?;
    Ok(TxnBuilder::with(
        params,
        CreateApplication::new(
            *sender,
            compiled_approval_program.clone().program,
            compiled_clear_program.program,
            StateSchema {
                number_ints: 0,
                number_byteslices: 0,
            },
            StateSchema {
                number_ints: 0,
                number_byteslices: 0,
            },
        )
        .build(),
    )
    .build())
}

#[allow(dead_code)]
#[cfg(test)]
pub async fn pay(algod: &Algod, sender: &Address) -> Result<Transaction> {
    let params = algod.suggested_transaction_params().await?;
    // sender sends a payment to themselves - don't need another party right now
    Ok(TxnBuilder::with(
        params.clone(),
        Pay::new(*sender, *sender, MicroAlgos(10_000)).build(),
    )
    .build())
}

/// This test doesn't mean anything anymore - it was written to check why app id wasn't being returned in a particular scenario
/// leaving the code there as "template" for possible similar tests
#[cfg(test)]
#[test]
async fn ad_hoc() -> Result<()> {
    let algod = algod();

    let sender = creator();

    let mut create_app_tx = create_always_approves_app(&algod, &sender.address()).await?;
    let mut pay_tx = pay(&algod, &sender.address()).await?;

    TxGroup::assign_group_id(vec![&mut create_app_tx, &mut pay_tx]).unwrap();

    let create_app_signed_tx = sender.sign_transaction(&create_app_tx)?;
    let pay_signed_tx = sender.sign_transaction(&pay_tx)?;

    let create_app_res = algod
        .broadcast_signed_transactions(&[create_app_signed_tx, pay_signed_tx])
        .await
        .unwrap();
    let p_tx = wait_for_pending_transaction(&algod, &create_app_res.tx_id)
        .await
        .unwrap()
        .unwrap();

    let app_id = p_tx.application_index;
    println!("app_id: {:?}", app_id);

    Ok(())
}
