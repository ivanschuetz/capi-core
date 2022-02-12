/// Some quick tests to confirm not documented Algorand functionality
/// [ignore] because we don't test Algorand here, this is so to say a documentation substitute.

#[cfg(test)]
use super::test_data::{creator, investor1};
#[cfg(test)]
use crate::testing::network_test_util::test_init;
#[cfg(test)]
use crate::{
    dependencies::algod_for_tests,
    network_util::wait_for_pending_transaction,
    state::app_state::{AppStateKey, ApplicationStateExt},
    teal::load_teal,
};
use algonaut::core::SuggestedTransactionParams;
#[cfg(test)]
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    transaction::{
        account::Account,
        builder::{CallApplication, OptInApplication},
        transaction::StateSchema,
        tx_group::TxGroup,
        AcceptAsset, CreateApplication, CreateAsset, Pay, Transaction, TransferAsset, TxnBuilder,
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
            compiled_approval_program.clone(),
            compiled_clear_program,
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

#[allow(dead_code)]
#[cfg(test)]
pub async fn optin_to_asset(algod: &Algod, sender: &Address, asset_id: u64) -> Result<Transaction> {
    let params = algod.suggested_transaction_params().await?;
    Ok(TxnBuilder::with(params, AcceptAsset::new(*sender, asset_id).build()).build())
}

#[allow(dead_code)]
#[cfg(test)]
pub async fn create_asset_tx(algod: &Algod, sender: &Address) -> Result<Transaction> {
    let params = algod.suggested_transaction_params().await?;
    Ok(TxnBuilder::with(
        params.clone(),
        CreateAsset::new(*sender, 1000, 0, false)
            .unit_name("FOO".to_owned())
            .asset_name("foo".to_owned())
            .build(),
    )
    .build())
}

#[allow(dead_code)]
#[cfg(test)]
pub async fn transfer_asset_tx(
    algod: &Algod,
    sender: &Address,
    receiver: &Address,
    asset_id: u64,
    amount: u64,
) -> Result<Transaction> {
    let params = algod.suggested_transaction_params().await?;
    Ok(TxnBuilder::with(
        params.clone(),
        TransferAsset::new(*sender, asset_id, amount, *receiver).build(),
    )
    .build())
}

#[allow(dead_code)]
#[cfg(test)]
pub async fn create_asset_and_sign(algod: &Algod, sender: &Account) -> Result<u64> {
    let create_asset_tx = create_asset_tx(&algod, &sender.address()).await?;
    let create_asset_signed_tx = sender.sign_transaction(&create_asset_tx)?;
    let create_asset_res = algod
        .broadcast_signed_transaction(&create_asset_signed_tx)
        .await?;
    let p_tx = wait_for_pending_transaction(&algod, &create_asset_res.tx_id.parse()?)
        .await?
        .unwrap();
    let asset_id = p_tx.asset_index.unwrap();
    Ok(asset_id)
}

#[allow(dead_code)]
#[cfg(test)]
pub async fn transfer_asset_and_sign(
    algod: &Algod,
    sender: &Account,
    receiver: &Address,
    asset_id: u64,
    amount: u64,
) -> Result<()> {
    let transfer_tx =
        transfer_asset_tx(&algod, &sender.address(), receiver, asset_id, amount).await?;
    let transfer_signed_tx = sender.sign_transaction(&transfer_tx)?;
    let transfer_res = algod
        .broadcast_signed_transaction(&transfer_signed_tx)
        .await?;
    wait_for_pending_transaction(&algod, &transfer_res.tx_id.parse()?).await?;
    Ok(())
}

#[cfg(test)]
#[test]
#[ignore]
async fn create_app_has_to_be_first_in_group_to_retrieve_app_id() -> Result<()> {
    let algod = algod_for_tests();

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
    let p_tx = wait_for_pending_transaction(&algod, &create_app_res.tx_id.parse()?)
        .await
        .unwrap()
        .unwrap();

    let app_id = p_tx.application_index;
    log::debug!("app_id: {:?}", app_id);

    Ok(())
}

#[cfg(test)]
#[test]
#[ignore]
async fn optin_and_receive_asset_can_be_in_the_same_group() -> Result<()> {
    let algod = algod_for_tests();

    let asset_creator_and_sender = creator();
    let assset_receiver = investor1();

    let asset_id = create_asset_and_sign(&algod, &asset_creator_and_sender).await?;

    let mut optin_to_asset_tx =
        optin_to_asset(&algod, &assset_receiver.address(), asset_id).await?;
    let mut receive_asset_tx = transfer_asset_tx(
        &algod,
        &asset_creator_and_sender.address(),
        &assset_receiver.address(),
        asset_id,
        100,
    )
    .await?;

    TxGroup::assign_group_id(vec![&mut optin_to_asset_tx, &mut receive_asset_tx]).unwrap();

    // asset receiver signs their optin
    let optin_to_asset_signed_tx = assset_receiver.sign_transaction(&optin_to_asset_tx)?;
    // asset creator/sender signs sending the asset to the receiver
    let receive_asset_signed_tx = asset_creator_and_sender.sign_transaction(&receive_asset_tx)?;

    let res = algod
        .broadcast_signed_transactions(&[optin_to_asset_signed_tx, receive_asset_signed_tx])
        .await;

    log::debug!("res: {:?}", res);
    assert!(res.is_ok());

    Ok(())
}

/// Weird? when opting in to an app and incrementing local state in a tx group, the state is incremented twice
/// is the smart contract being executed twice? The debugger shows only one execution (and increment).
#[cfg(test)]
#[test]
#[ignore]
async fn app_optin_and_local_state_access_in_same_group_increments_state_twice() -> Result<()> {
    test_init()?;

    let algod = algod_for_tests();

    let creator = creator();

    let params = algod.suggested_transaction_params().await?;

    let app_id =
        create_increment_local_state_app(&algod, &creator, &params, "increment_local_state_twice")
            .await?;

    let mut optin_tx = TxnBuilder::with(
        params.clone(),
        // TODO: investigate: Using CallApplication here instead of OptInApplication opts in the user too. Is this expected? Reviewed that the SDK is sending the correct integer.
        // CallApplication::new(caller.address(), app_id).app_arguments(vec!["opt_in".as_bytes().to_vec()]).build(),
        OptInApplication::new(creator.address(), app_id)
            .app_arguments(vec!["opt_in".as_bytes().to_vec()])
            .build(),
    )
    .build();
    let mut write_local_state_tx = TxnBuilder::with(
        params,
        CallApplication::new(creator.address(), app_id)
            .app_arguments(vec!["write_local_state".as_bytes().to_vec()])
            .build(),
    )
    .build();

    TxGroup::assign_group_id(vec![&mut optin_tx, &mut write_local_state_tx]).unwrap();

    let signed_optin_tx = creator.sign_transaction(&optin_tx)?;
    let signed_write_local_state_tx = creator.sign_transaction(&write_local_state_tx)?;

    let res = algod
        .broadcast_signed_transactions(&[signed_optin_tx, signed_write_local_state_tx])
        .await?;

    wait_for_pending_transaction(&algod, &res.tx_id.parse()?)
        .await?
        .unwrap();

    let local_state =
        crate::state::app_state::local_state(&algod, &creator.address(), app_id).await?;

    let incremented_value = local_state.find_uint(&AppStateKey("MyLocalState"));

    // this is the problem: we do only +10 in TEAL, but it's executed 2x, so 20
    assert_eq!(incremented_value.unwrap(), 20);

    Ok(())
}

#[cfg(test)]
#[test]
#[ignore]
async fn app_optin_and_local_state_access_in_separate_groups_increments_state_once() -> Result<()> {
    test_init()?;

    let algod = algod_for_tests();

    let creator = creator();

    let params = algod.suggested_transaction_params().await?;

    let app_id =
        create_increment_local_state_app(&algod, &creator, &params, "increment_local_state_once")
            .await?;

    let optin_tx = TxnBuilder::with(
        params.clone(),
        // TODO: investigate: Using CallApplication here instead of OptInApplication opts in the user too. Is this expected? Reviewed that the SDK is sending the correct integer.
        // CallApplication::new(caller.address(), app_id).app_arguments(vec!["opt_in".as_bytes().to_vec()]).build(),
        OptInApplication::new(creator.address(), app_id)
            .app_arguments(vec!["opt_in".as_bytes().to_vec()])
            .build(),
    )
    .build();

    let write_local_state_tx = TxnBuilder::with(
        params,
        CallApplication::new(creator.address(), app_id)
            .app_arguments(vec!["write_local_state".as_bytes().to_vec()])
            .build(),
    )
    .build();

    let signed_optin_tx = creator.sign_transaction(&optin_tx)?;
    let signed_write_local_state_tx = creator.sign_transaction(&write_local_state_tx)?;

    let res = algod.broadcast_signed_transaction(&signed_optin_tx).await?;

    wait_for_pending_transaction(&algod, &res.tx_id.parse()?)
        .await?
        .unwrap();

    let res = algod
        .broadcast_signed_transaction(&signed_write_local_state_tx)
        .await?;

    wait_for_pending_transaction(&algod, &res.tx_id.parse()?)
        .await?
        .unwrap();

    let local_state =
        crate::state::app_state::local_state(&algod, &creator.address(), app_id).await?;

    let incremented_value = local_state.find_uint(&AppStateKey("MyLocalState"));

    // the state is incremented only once, as expected
    assert_eq!(incremented_value.unwrap(), 10);

    Ok(())
}

/// Returns created app id
async fn create_increment_local_state_app(
    algod: &Algod,
    sender: &Account,
    params: &SuggestedTransactionParams,
    teal_file_name: &str,
) -> Result<u64> {
    let teal_source = load_teal(teal_file_name)?;
    let compiled_approval_program = algod.compile_teal(&teal_source.0).await?;
    let compiled_clear_program = algod.compile_teal(&teal_source.0).await?;

    let create_app_tx = TxnBuilder::with(
        params.clone(),
        CreateApplication::new(
            sender.address(),
            compiled_approval_program.clone(),
            compiled_clear_program,
            StateSchema {
                number_ints: 0,
                number_byteslices: 0,
            },
            StateSchema {
                number_ints: 1,
                number_byteslices: 0,
            },
        )
        .build(),
    )
    .build();
    let create_app_signed_tx = sender.sign_transaction(&create_app_tx)?;
    let create_app_res = algod
        .broadcast_signed_transaction(&create_app_signed_tx)
        .await?;
    let p_tx = wait_for_pending_transaction(&algod, &create_app_res.tx_id.parse()?)
        .await?
        .unwrap();

    Ok(p_tx.application_index.unwrap())
}
