use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, builder::CallApplication, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn withdraw(
    algod: &Algod,
    creator: Address,
    amount: MicroAlgos,
    central_escrow: &ContractAccount,
    slot_app_id: u64,
) -> Result<WithdrawToSign> {
    log::debug!("Creating withdrawal txs..");

    let params = algod.suggested_transaction_params().await?;

    // Slot app call to validate vote count
    let mut check_enough_votes_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(creator, slot_app_id)
            .app_arguments(vec!["branch_withdraw".as_bytes().to_vec()])
            .build(),
    )
    .build();

    // Funds transfer from escrow to creator
    let mut withdraw_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(central_escrow.address, creator, amount).build(),
    )
    .build();

    // The creator pays the fee of the withdraw tx (signed by central escrow)
    let mut pay_withdraw_fee_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(creator, central_escrow.address, FIXED_FEE).build(),
    )
    .build();

    TxGroup::assign_group_id(vec![
        &mut check_enough_votes_tx,
        &mut withdraw_tx,
        &mut pay_withdraw_fee_tx,
    ])?;

    let signed_withdraw_tx = central_escrow.sign(&withdraw_tx, vec![])?;

    Ok(WithdrawToSign {
        check_enough_votes_tx,
        withdraw_tx: signed_withdraw_tx,
        pay_withdraw_fee_tx,
    })
}

pub async fn submit_withdraw(algod: &Algod, signed: &WithdrawSigned) -> Result<String> {
    log::debug!("Submit withdrawal txs..");

    let txs = vec![
        signed.check_enough_votes_tx.clone(),
        signed.withdraw_tx.clone(),
        signed.pay_withdraw_fee_tx.clone(),
    ];

    // crate::teal::debug_teal_rendered(&txs, "central_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "withdrawal_slot_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Withdrawal txs tx id: {}", res.tx_id);

    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawToSign {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: Transaction,
    pub check_enough_votes_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawSigned {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: SignedTransaction,
    pub check_enough_votes_tx: SignedTransaction,
}

#[cfg(test)]
mod tests {
    use algonaut::{
        algod::v2::Algod,
        core::{Address, MicroAlgos},
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::withdraw::logic::{submit_withdraw, withdraw, WithdrawSigned, FIXED_FEE},
        network_util::wait_for_pending_transaction,
        testing::{
            flow::{
                create_project::create_project_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
                init_withdrawal::init_withdrawal_flow,
                invest_in_project::{invests_flow, invests_optins_flow},
                vote::vote_flow,
                withdraw::{withdraw_flow, withdraw_precs},
            },
            network_test_util::reset_network,
            test_data::{creator, customer, investor1, investor2, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
        withdrawal_app_state::{
            votes_global_state, withdrawal_amount_global_state, withdrawal_round_global_state,
        },
    };

    #[test]
    #[serial]
    async fn test_init_withdrawal_app_state_correct() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            3,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        // select arbitrary slot
        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // flow

        let init_withdrawal_tx_id =
            init_withdrawal_flow(&algod, &creator, withdraw_amount, slot_id).await?;
        wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

        // test

        let slot_app = algod.application_information(slot_id).await?;

        let withdrawal_amount = withdrawal_amount_global_state(&slot_app);
        assert_eq!(Some(withdraw_amount.0), withdrawal_amount);

        let vote_count = votes_global_state(&slot_app);
        assert_eq!(None, vote_count);

        let withdrawal_round = withdrawal_round_global_state(&slot_app);
        assert_eq!(Some(1), withdrawal_round);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_withdraw_success() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let drainer = investor1();
        let voter = investor2();
        let customer = customer();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            3,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);
        // select arbitrary slot
        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        withdraw_precs(
            &algod,
            &creator,
            &drainer,
            &customer,
            &voter,
            &project,
            pay_and_drain_amount,
            withdraw_amount,
            slot_id,
        )
        .await?;

        // remeber state
        let central_balance_before_withdrawing = algod
            .account_information(&project.central_escrow.address)
            .await?
            .amount;
        let creator_balance_bafore_withdrawing =
            algod.account_information(&creator.address()).await?.amount;

        // flow

        withdraw_flow(&algod, &project, &creator, withdraw_amount, slot_id).await?;

        // test

        after_withdrawal_success_or_failure_tests(
            &algod,
            &creator.address(),
            &project.central_escrow.address,
            slot_id,
            // creator got the amount and lost the fees for the withdraw txs (app call, pay escrow fee and fee of that tx)
            creator_balance_bafore_withdrawing + withdraw_amount - FIXED_FEE * 3,
            // central lost the withdrawn amount
            central_balance_before_withdrawing - withdraw_amount,
            // amount reset to 0
            Some(0),
            // votes reset to 0
            Some(0),
        )
        .await
    }

    #[test]
    #[serial]
    async fn test_withdraw_without_active_request_fails() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let drainer = investor1();
        let voter = investor2();
        let customer = customer();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            3,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);
        // select arbitrary slot
        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // customer payment and draining, to have some funds to withdraw
        customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            pay_and_drain_amount,
            &project,
        )
        .await?;

        // Investor buys shares with count == vote threshold count
        let investor_shares_count = project.specs.vote_threshold_units();
        invests_optins_flow(&algod, &voter, &project).await?;
        invests_flow(&algod, &voter, investor_shares_count, &project).await?;

        // remeber state
        let central_balance_before_withdrawing = algod
            .account_information(&project.central_escrow.address)
            .await?
            .amount;
        let creator_balance_bafore_withdrawing =
            algod.account_information(&creator.address()).await?.amount;

        // flow

        let to_sign = withdraw(
            &algod,
            creator.address(),
            withdraw_amount,
            &project.central_escrow,
            slot_id,
        )
        .await?;

        // UI
        let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;
        let check_enough_votes_tx_signed =
            creator.sign_transaction(&to_sign.check_enough_votes_tx)?;

        let withdraw_res = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
                check_enough_votes_tx: check_enough_votes_tx_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            &algod,
            &creator.address(),
            &project.central_escrow.address,
            slot_id,
            creator_balance_bafore_withdrawing,
            central_balance_before_withdrawing,
            None,
            None,
        )
        .await
    }

    #[test]
    #[serial]
    async fn test_withdraw_without_enough_votes_fails() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let drainer = investor1();
        let voter = investor2();
        let customer = customer();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            3,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);
        // select arbitrary slot
        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // customer payment and draining, to have some funds to withdraw
        customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            pay_and_drain_amount,
            &project,
        )
        .await?;

        // Investor buys shares with count < vote threshold count
        assert!(project.specs.vote_threshold_units() > 2); // sanity check (2 specifically because 1-1=0 which could trigger different conditions)
        let investor_shares_count = project.specs.vote_threshold_units() - 1;
        invests_optins_flow(&algod, &voter, &project).await?;
        invests_flow(&algod, &voter, investor_shares_count, &project).await?;

        // Init a request
        let init_withdrawal_tx_id =
            init_withdrawal_flow(&algod, &creator, withdraw_amount, slot_id).await?;
        wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

        // Vote
        let vote_tx_id =
            vote_flow(&algod, &voter, &project, slot_id, investor_shares_count).await?;
        wait_for_pending_transaction(&algod, &vote_tx_id).await?;

        // remeber state
        let central_balance_before_withdrawing = algod
            .account_information(&project.central_escrow.address)
            .await?
            .amount;
        let creator_balance_bafore_withdrawing =
            algod.account_information(&creator.address()).await?.amount;

        // flow

        let to_sign = withdraw(
            &algod,
            creator.address(),
            withdraw_amount,
            &project.central_escrow,
            slot_id,
        )
        .await?;

        // UI
        let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;
        let check_enough_votes_tx_signed =
            creator.sign_transaction(&to_sign.check_enough_votes_tx)?;

        let withdraw_res = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
                check_enough_votes_tx: check_enough_votes_tx_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            &algod,
            &creator.address(),
            &project.central_escrow.address,
            slot_id,
            creator_balance_bafore_withdrawing,
            central_balance_before_withdrawing,
            Some(withdraw_amount.0),
            Some(investor_shares_count),
        )
        .await
    }

    #[test]
    #[serial]
    async fn test_withdraw_without_enough_funds_fails() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let voter = investor2();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            3,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        // select arbitrary slot
        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // Investor buys shares with count == vote threshold count
        let investor_shares_count = project.specs.vote_threshold_units();
        invests_optins_flow(&algod, &voter, &project).await?;
        invests_flow(&algod, &voter, investor_shares_count, &project).await?;

        // Init a request
        let init_withdrawal_tx_id =
            init_withdrawal_flow(&algod, &creator, withdraw_amount, slot_id).await?;
        wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

        // Vote
        let vote_tx_id =
            vote_flow(&algod, &voter, &project, slot_id, investor_shares_count).await?;
        wait_for_pending_transaction(&algod, &vote_tx_id).await?;

        // remeber state
        let central_balance_before_withdrawing = algod
            .account_information(&project.central_escrow.address)
            .await?
            .amount;
        let creator_balance_bafore_withdrawing =
            algod.account_information(&creator.address()).await?.amount;

        // flow

        let to_sign = withdraw(
            &algod,
            creator.address(),
            withdraw_amount,
            &project.central_escrow,
            slot_id,
        )
        .await?;

        // UI
        let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;
        let check_enough_votes_tx_signed =
            creator.sign_transaction(&to_sign.check_enough_votes_tx)?;

        let withdraw_res = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
                check_enough_votes_tx: check_enough_votes_tx_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            &algod,
            &creator.address(),
            &project.central_escrow.address,
            slot_id,
            creator_balance_bafore_withdrawing,
            central_balance_before_withdrawing,
            Some(withdraw_amount.0),
            Some(investor_shares_count),
        )
        .await
    }

    #[test]
    #[serial]
    async fn test_withdraw_by_not_creator_fails() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let drainer = investor1();
        let voter = investor2();
        let customer = customer();
        let not_creator = investor2();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            3,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);
        // select arbitrary slot
        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // customer payment and draining, to have some funds to withdraw
        customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            pay_and_drain_amount,
            &project,
        )
        .await?;

        // Investor buys shares with count < vote threshold count
        let investor_shares_count = project.specs.vote_threshold_units();
        invests_optins_flow(&algod, &voter, &project).await?;
        invests_flow(&algod, &voter, investor_shares_count, &project).await?;

        // Init a request
        let init_withdrawal_tx_id =
            init_withdrawal_flow(&algod, &creator, withdraw_amount, slot_id).await?;
        wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

        // Vote
        let vote_tx_id =
            vote_flow(&algod, &voter, &project, slot_id, investor_shares_count).await?;
        wait_for_pending_transaction(&algod, &vote_tx_id).await?;

        // remeber state
        let central_balance_before_withdrawing = algod
            .account_information(&project.central_escrow.address)
            .await?
            .amount;
        let creator_balance_bafore_withdrawing =
            algod.account_information(&creator.address()).await?.amount;

        // flow

        let to_sign = withdraw(
            &algod,
            not_creator.address(),
            withdraw_amount,
            &project.central_escrow,
            slot_id,
        )
        .await?;

        // UI
        let pay_withdraw_fee_tx_signed =
            not_creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;
        let check_enough_votes_tx_signed =
            not_creator.sign_transaction(&to_sign.check_enough_votes_tx)?;

        let withdraw_res = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
                check_enough_votes_tx: check_enough_votes_tx_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            &algod,
            &creator.address(),
            &project.central_escrow.address,
            slot_id,
            creator_balance_bafore_withdrawing,
            central_balance_before_withdrawing,
            Some(withdraw_amount.0),
            Some(investor_shares_count),
        )
        .await
    }

    #[test]
    #[serial]
    async fn test_increments_withdrawal_round_when_creating_new_request() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let drainer = investor1();
        let voter = investor2();
        let customer = customer();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project = create_project_flow(
            &algod,
            &creator,
            &project_specs(),
            3,
            TESTS_DEFAULT_PRECISION,
        )
        .await?;
        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);
        // select arbitrary slot
        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // withdraw
        let _ = withdraw_precs(
            &algod,
            &creator,
            &drainer,
            &customer,
            &voter,
            &project,
            pay_and_drain_amount,
            withdraw_amount,
            slot_id,
        )
        .await?;
        withdraw_flow(&algod, &project, &creator, withdraw_amount, slot_id).await?;

        // flow

        // init a new withdrawal request
        let new_withdraw_amount = MicroAlgos(1_123_123); // UI
        let init_withdrawal_tx_id =
            init_withdrawal_flow(&algod, &creator, new_withdraw_amount, slot_id).await?;
        wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

        // test

        let slot_app = algod.application_information(slot_id).await?;

        // double check: new request amount
        let withdrawal_amount = withdrawal_amount_global_state(&slot_app);
        assert_eq!(Some(new_withdraw_amount.0), withdrawal_amount);
        // double check: 0 votes (value was reset and no one has voted yet)
        let vote_count = votes_global_state(&slot_app);
        assert_eq!(Some(0), vote_count);

        // we initiated a second round, so round global state is now 2
        let withdrawal_round = withdrawal_round_global_state(&slot_app);
        assert_eq!(Some(2), withdrawal_round);

        Ok(())
    }

    async fn test_withdrawal_did_not_succeed(
        algod: &Algod,
        creator_address: &Address,
        central_escrow_address: &Address,
        slot_id: u64,
        creator_balance_before_withdrawing: MicroAlgos,
        central_balance_before_withdrawing: MicroAlgos,
        withdraw_amount_global_state_before_withdrawing: Option<u64>,
        votes_global_state_before_withdrawing: Option<u64>,
    ) -> Result<()> {
        after_withdrawal_success_or_failure_tests(
            algod,
            creator_address,
            central_escrow_address,
            slot_id,
            creator_balance_before_withdrawing,
            central_balance_before_withdrawing,
            withdraw_amount_global_state_before_withdrawing,
            votes_global_state_before_withdrawing,
        )
        .await
    }

    async fn after_withdrawal_success_or_failure_tests(
        algod: &Algod,
        creator_address: &Address,
        central_escrow_address: &Address,
        slot_id: u64,
        expected_withdrawer_balance: MicroAlgos,
        expected_central_balance: MicroAlgos,
        // TODO option (at the beginning) vs 0 (default value we set when a withdrawal succeeds)
        // can we use only 1 (probably 0, i.e. we've to init to 0 on setup) -- also for votes and other global state
        expected_withdraw_amount_global_state: Option<u64>,
        expected_votes_global_state: Option<u64>,
    ) -> Result<()> {
        // check creator's balance
        let withdrawer_account = algod.account_information(&creator_address).await?;
        assert_eq!(expected_withdrawer_balance, withdrawer_account.amount);

        // check central's balance
        let central_escrow_balance = algod
            .account_information(central_escrow_address)
            .await?
            .amount;
        assert_eq!(expected_central_balance, central_escrow_balance);

        let slot_app = algod.application_information(slot_id).await?;

        // check slot app withdrawal amount
        let withdrawal_amount = withdrawal_amount_global_state(&slot_app);
        assert_eq!(expected_withdraw_amount_global_state, withdrawal_amount);

        // check slot app votes count
        let vote_count = votes_global_state(&slot_app);
        assert_eq!(expected_votes_global_state, vote_count);

        Ok(())
    }
}
