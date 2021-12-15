use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, tx_group::TxGroup, Pay, SignedTransaction, Transaction,
        TxnBuilder,
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
) -> Result<WithdrawToSign> {
    log::debug!("Creating withdrawal txs..");

    let params = algod.suggested_transaction_params().await?;

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
            ..params
        },
        Pay::new(creator, central_escrow.address, FIXED_FEE).build(),
    )
    .build();

    TxGroup::assign_group_id(vec![&mut withdraw_tx, &mut pay_withdraw_fee_tx])?;

    let signed_withdraw_tx = central_escrow.sign(&withdraw_tx, vec![])?;

    Ok(WithdrawToSign {
        withdraw_tx: signed_withdraw_tx,
        pay_withdraw_fee_tx,
    })
}

pub async fn submit_withdraw(algod: &Algod, signed: &WithdrawSigned) -> Result<String> {
    log::debug!("Submit withdrawal txs..");

    let txs = vec![
        signed.withdraw_tx.clone(),
        signed.pay_withdraw_fee_tx.clone(),
    ];

    // crate::teal::debug_teal_rendered(&txs, "central_escrow").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    log::debug!("Withdrawal txs tx id: {}", res.tx_id);

    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawToSign {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawSigned {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: SignedTransaction,
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
        testing::{
            flow::{
                create_project::create_project_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
                invest_in_project::{invests_flow, invests_optins_flow},
                withdraw::{withdraw_flow, withdraw_precs},
            },
            network_test_util::reset_network,
            test_data::{creator, customer, investor1, investor2, project_specs},
            TESTS_DEFAULT_PRECISION,
        },
    };

    #[test]
    #[serial]
    async fn test_withdraw_success() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let drainer = investor1();
        let customer = customer();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project =
            create_project_flow(&algod, &creator, &project_specs(), TESTS_DEFAULT_PRECISION)
                .await?;
        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);

        withdraw_precs(&algod, &drainer, &customer, &project, pay_and_drain_amount).await?;

        // remeber state
        let central_balance_before_withdrawing = algod
            .account_information(&project.central_escrow.address)
            .await?
            .amount;
        let creator_balance_bafore_withdrawing =
            algod.account_information(&creator.address()).await?.amount;

        // flow

        withdraw_flow(&algod, &project, &creator, withdraw_amount).await?;

        // test

        after_withdrawal_success_or_failure_tests(
            &algod,
            &creator.address(),
            &project.central_escrow.address,
            // creator got the amount and lost the fees for the withdraw txs (pay escrow fee and fee of that tx)
            creator_balance_bafore_withdrawing + withdraw_amount - FIXED_FEE * 2,
            // central lost the withdrawn amount
            central_balance_before_withdrawing - withdraw_amount,
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
        let investor = investor1();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project =
            create_project_flow(&algod, &creator, &project_specs(), TESTS_DEFAULT_PRECISION)
                .await?;

        // Investor buys some shares
        let investor_shares_count = 10;
        invests_optins_flow(&algod, &investor, &project).await?;
        invests_flow(&algod, &investor, investor_shares_count, &project).await?;

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
        )
        .await?;

        // UI
        let pay_withdraw_fee_tx_signed = creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;

        let withdraw_res = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            &algod,
            &creator.address(),
            &project.central_escrow.address,
            creator_balance_bafore_withdrawing,
            central_balance_before_withdrawing,
        )
        .await
    }

    // TODO: test is failing after removing governance - add creator check to central escrow
    #[test]
    #[serial]
    async fn test_withdraw_by_not_creator_fails() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let drainer = investor1();
        let investor = investor2();
        let customer = customer();
        let not_creator = investor2();

        // precs

        let withdraw_amount = MicroAlgos(1_000_000); // UI

        let project =
            create_project_flow(&algod, &creator, &project_specs(), TESTS_DEFAULT_PRECISION)
                .await?;
        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);

        // customer payment and draining, to have some funds to withdraw
        customer_payment_and_drain_flow(
            &algod,
            &drainer,
            &customer,
            pay_and_drain_amount,
            &project,
        )
        .await?;

        // Investor buys some shares
        let investor_shares_count = 10;
        invests_optins_flow(&algod, &investor, &project).await?;
        invests_flow(&algod, &investor, investor_shares_count, &project).await?;

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
        )
        .await?;

        // UI
        let pay_withdraw_fee_tx_signed =
            not_creator.sign_transaction(&to_sign.pay_withdraw_fee_tx)?;

        let withdraw_res = submit_withdraw(
            &algod,
            &WithdrawSigned {
                withdraw_tx: to_sign.withdraw_tx,
                pay_withdraw_fee_tx: pay_withdraw_fee_tx_signed,
            },
        )
        .await;

        // test

        assert!(withdraw_res.is_err());

        test_withdrawal_did_not_succeed(
            &algod,
            &creator.address(),
            &project.central_escrow.address,
            creator_balance_bafore_withdrawing,
            central_balance_before_withdrawing,
        )
        .await
    }

    async fn test_withdrawal_did_not_succeed(
        algod: &Algod,
        creator_address: &Address,
        central_escrow_address: &Address,
        creator_balance_before_withdrawing: MicroAlgos,
        central_balance_before_withdrawing: MicroAlgos,
    ) -> Result<()> {
        after_withdrawal_success_or_failure_tests(
            algod,
            creator_address,
            central_escrow_address,
            creator_balance_before_withdrawing,
            central_balance_before_withdrawing,
        )
        .await
    }

    async fn after_withdrawal_success_or_failure_tests(
        algod: &Algod,
        creator_address: &Address,
        central_escrow_address: &Address,
        expected_withdrawer_balance: MicroAlgos,
        expected_central_balance: MicroAlgos,
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

        Ok(())
    }
}
