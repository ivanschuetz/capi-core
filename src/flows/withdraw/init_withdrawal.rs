use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{builder::CallApplication, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn init_withdrawal(
    algod: &Algod,
    creator: &Address,
    amount: MicroAlgos,
    withdrawal_slot: u64,
) -> Result<InitWithdrawalToSign> {
    let params = algod.suggested_transaction_params().await?;

    // Initializes a withdrawal request, by setting the amount. Fails if there's already an active request (amount > 0 in the slot.
    let init_withdrawal_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(*creator, withdrawal_slot)
            .app_arguments(vec![
                "branch_init_request".as_bytes().to_vec(),
                amount.0.to_be_bytes().to_vec(),
            ])
            .build(),
    )
    .build();

    Ok(InitWithdrawalToSign {
        init_withdrawal_slot_app_call_tx: init_withdrawal_tx,
    })
}

pub async fn submit_init_withdrawal(
    algod: &Algod,
    signed: &InitWithdrawalSigned,
) -> Result<String> {
    // crate::teal::debug_teal_rendered(
    //     &[signed.init_withdrawal_slot_app_call_tx.clone()],
    //     "withdrawal_slot_approval",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[signed.init_withdrawal_slot_app_call_tx.clone()])
        .await?;
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitWithdrawalToSign {
    pub init_withdrawal_slot_app_call_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitWithdrawalSigned {
    pub init_withdrawal_slot_app_call_tx: SignedTransaction,
}

#[cfg(test)]
mod tests {
    use algonaut::core::MicroAlgos;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        network_util::wait_for_pending_transaction,
        testing::{
            flow::{create_project::create_project_flow, init_withdrawal::init_withdrawal_flow},
            network_test_util::reset_network,
            test_data::{creator, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
        withdrawal_app_state::{votes_global_state, withdrawal_amount_global_state},
    };

    #[test]
    #[serial]
    async fn test_init_withdraw() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();

        // UI
        let specs = project_specs();
        let project =
            create_project_flow(&algod, &creator, &specs, 3, TESTS_DEFAULT_PRECISION).await?;

        let amount_to_withdraw = MicroAlgos(123456789);

        // flow

        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // double check that initial amount is None (we're initializing a withdrawal in a slot without active requests)
        // TODO review this: None vs. default value (0): should None be the default value, can we set state to None? or should be set 0 on init?
        let slot_app = algod.application_information(slot_id).await?;
        let initial_withdrawal_amount = withdrawal_amount_global_state(&slot_app);
        assert!(initial_withdrawal_amount.is_none());
        // assert!(initial_withdrawal_amount.is_some());
        // assert_eq!(0, initial_withdrawal_amount.unwrap());

        // double check votes initial value
        let slot_app = algod.application_information(slot_id).await?;
        let initial_votes = votes_global_state(&slot_app);
        assert!(initial_votes.is_none());
        // assert!(initial_votes.is_some());
        // assert_eq!(0, initial_votes.unwrap());

        let tx_id = init_withdrawal_flow(&algod, &creator, amount_to_withdraw, slot_id).await?;
        let _ = wait_for_pending_transaction(&algod, &tx_id).await?;

        // test

        // check that amount is what we set
        let slot_app = algod.application_information(slot_id).await?;
        let request_withdrawal_amount = withdrawal_amount_global_state(&slot_app);
        assert!(request_withdrawal_amount.is_some());
        assert_eq!(
            amount_to_withdraw,
            MicroAlgos(request_withdrawal_amount.unwrap())
        );

        // check that initializing withdrawal amount doesn't affect votes (and that votes are the initial/default value)
        let slot_app = algod.application_information(slot_id).await?;
        let votes = votes_global_state(&slot_app);
        assert!(votes.is_none());
        // assert!(votes.is_some());
        // assert_eq!(0, votes.unwrap());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cannot_init_withdraw_if_already_active() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();

        // UI
        let specs = project_specs();
        let project =
            create_project_flow(&algod, &creator, &specs, 3, TESTS_DEFAULT_PRECISION).await?;

        let amount_to_withdraw = MicroAlgos(123456789);

        // flow

        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // double check that initial amount is None (we're initializing a withdrawal in a slot without active requests)
        // TODO review this: None vs. default value (0): should None be the default value, can we set state to None? or should be set 0 on init?
        let slot_app = algod.application_information(slot_id).await?;
        let initial_withdrawal_amount = withdrawal_amount_global_state(&slot_app);
        assert!(initial_withdrawal_amount.is_none());
        // assert!(initial_withdrawal_amount.is_some());
        // assert_eq!(0, initial_withdrawal_amount.unwrap());

        // double check votes initial value
        let slot_app = algod.application_information(slot_id).await?;
        let initial_votes = votes_global_state(&slot_app);
        assert!(initial_votes.is_none());
        // assert!(initial_votes.is_some());
        // assert_eq!(0, initial_votes.unwrap());

        let tx_id = init_withdrawal_flow(&algod, &creator, amount_to_withdraw, slot_id).await?;
        let _ = wait_for_pending_transaction(&algod, &tx_id).await?;

        // flow + test

        // submitting a new request fails
        let new_init_withdrawal_res =
            init_withdrawal_flow(&algod, &creator, MicroAlgos(123), slot_id).await;
        println!("Expected error: {:?}", new_init_withdrawal_res);
        assert!(new_init_withdrawal_res.is_err());

        // submitting a new request fails - testing with 0, just in case, as 0 _can_ have a different meaning (e.g. default/inactive value)
        let new_init_withdrawal_res =
            init_withdrawal_flow(&algod, &creator, MicroAlgos(0), slot_id).await;
        println!("Expected error: {:?}", new_init_withdrawal_res);
        assert!(new_init_withdrawal_res.is_err());

        Ok(())
    }
}
