use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    model::algod::v2::{Account, AssetHolding},
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
    use algonaut::core::MicroAlgos;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::withdraw::logic::FIXED_FEE,
        testing::{
            flow::{
                create_project::create_project_flow,
                withdraw::{withdraw_flow, withdraw_precs},
            },
            network_test_util::reset_network,
            test_data::{creator, customer, investor1, investor2, project_specs},
        },
        withdrawal_app_state::{votes_global_state, withdrawal_amount_global_state},
    };

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

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);
        withdraw_precs(
            &algod,
            &creator,
            &drainer,
            &customer,
            &voter,
            &project,
            pay_and_drain_amount,
            withdraw_amount,
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

        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];
        let _res = withdraw_flow(&algod, &project, &creator, withdraw_amount, slot_id).await?;

        // test

        // creator got the amount and lost the fees for the withdraw txs (app call, pay escrow fee and fee of that tx)
        let withdrawer_account = algod.account_information(&creator.address()).await?;
        assert_eq!(
            creator_balance_bafore_withdrawing + withdraw_amount - FIXED_FEE * 3,
            withdrawer_account.amount
        );

        // central lost the withdrawn amount
        let central_escrow_balance = algod
            .account_information(&project.central_escrow.address)
            .await?
            .amount;
        assert_eq!(
            central_balance_before_withdrawing - withdraw_amount,
            central_escrow_balance
        );

        // slot app reset amount to 0
        let slot_app = algod.application_information(slot_id).await?;
        let initial_withdrawal_amount = withdrawal_amount_global_state(&slot_app);
        assert_eq!(Some(0), initial_withdrawal_amount);

        // slot app reset votes to 0
        let slot_app = algod.application_information(slot_id).await?;
        let initial_vote_count = votes_global_state(&slot_app);
        assert_eq!(Some(0), initial_vote_count);

        Ok(())
    }

    // TODO test for failing case (not enough votes)
}

trait AssetHolder {
    fn get_holdings(&self, asset_id: u64) -> Option<AssetHolding>;
}

impl AssetHolder for Account {
    fn get_holdings(&self, asset_id: u64) -> Option<AssetHolding> {
        self.assets
            .clone()
            .into_iter()
            .find(|a| a.asset_id == asset_id)
    }
}
