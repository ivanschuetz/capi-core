use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::CallApplication, tx_group::TxGroup, SignedTransaction, Transaction, TxnBuilder,
    },
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn vote(
    algod: &Algod,
    voter: Address,
    central_app_id: u64,
    slot_app_id: u64,
    votes_count: u64, // teal expects this to be == shares in local state
) -> Result<VoteToSign> {
    let params = algod.suggested_transaction_params().await?;

    // Slot app call to increment votes
    let mut vote_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(voter, slot_app_id)
            .app_arguments(vec![
                "branch_vote".as_bytes().to_vec(),
                votes_count.to_be_bytes().to_vec(),
            ])
            .build(),
    )
    .build();

    // Central app call to validate vote count (votes == to owned shares)
    let mut validate_vote_count_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(voter, central_app_id)
            .app_arguments(vec!["validate_vote".as_bytes().to_vec()])
            .build(),
    )
    .build();

    TxGroup::assign_group_id(vec![&mut vote_tx, &mut validate_vote_count_tx])?;

    Ok(VoteToSign {
        validate_vote_count_tx,
        vote_tx,
    })
}

pub async fn submit_vote(algod: &Algod, signed: &VoteSigned) -> Result<String> {
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.vote_tx.clone(),
    //         signed.validate_vote_count_tx.clone(),
    //     ],
    //     "withdrawal_slot_approval",
    // )
    // .unwrap();
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.vote_tx.clone(),
    //         signed.validate_vote_count_tx.clone(),
    //     ],
    //     "app_central_approval",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[
            signed.vote_tx.clone(),
            signed.validate_vote_count_tx.clone(),
        ])
        .await?;
    println!("Vote tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteToSign {
    pub vote_tx: Transaction,
    pub validate_vote_count_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteSigned {
    pub vote_tx: SignedTransaction,
    pub validate_vote_count_tx: SignedTransaction,
}

#[cfg(test)]
mod tests {
    use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::create_project::model::Project,
        network_util::wait_for_pending_transaction,
        testing::{
            flow::{
                create_project::create_project_flow, init_withdrawal::init_withdrawal_flow,
                invest_in_project::invests_flow, vote::vote_flow,
            },
            network_test_util::reset_network,
            test_data::{creator, investor1, project_specs},
        },
        withdrawal_app_state::{
            votes_global_state, votes_global_state_or_err, votes_local_state_or_err,
            withdrawal_amount_global_state,
        },
    };

    #[test]
    #[serial]
    async fn test_vote_succeeds() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        let buy_asset_amount = 10;
        let slot_id = test_vote_with_active_withdrawal_request_precs(
            &algod,
            &creator,
            &investor,
            &project,
            buy_asset_amount,
        )
        .await?;

        // flow

        let tx_id = vote_flow(&algod, &investor, &project, slot_id, buy_asset_amount).await?;
        wait_for_pending_transaction(&algod, &tx_id).await?;

        // test

        // check that votes global state was incremented correctly
        let slot_app = algod.application_information(slot_id).await?;
        let vote_amount = votes_global_state_or_err(&slot_app)?;
        assert_eq!(buy_asset_amount, vote_amount);

        // check that votes local state was set to voted (owned shares) amount
        let account = algod.account_information(&investor.address()).await?;
        let local_vote_amount = votes_local_state_or_err(&account.apps_local_state, slot_id)?;
        assert_eq!(buy_asset_amount, local_vote_amount);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_vote_fails_if_already_voted() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        let buy_asset_amount = 10;
        let slot_id = test_vote_with_active_withdrawal_request_precs(
            &algod,
            &creator,
            &investor,
            &project,
            buy_asset_amount,
        )
        .await?;

        // flow

        // vote 1st time: success
        let tx_id = vote_flow(&algod, &investor, &project, slot_id, buy_asset_amount).await?;
        wait_for_pending_transaction(&algod, &tx_id).await?;

        // vote 2nd time: fails
        let res = vote_flow(&algod, &investor, &project, slot_id, buy_asset_amount).await;
        assert!(res.is_err());

        // test

        // check that votes global state is as expected after 1st vote
        let slot_app = algod.application_information(slot_id).await?;
        let vote_amount = votes_global_state_or_err(&slot_app)?;
        assert_eq!(buy_asset_amount, vote_amount);

        // check that votes local state is as expected after 1st vote
        let account = algod.account_information(&investor.address()).await?;
        let local_vote_amount = votes_local_state_or_err(&account.apps_local_state, slot_id)?;
        assert_eq!(buy_asset_amount, local_vote_amount);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_vote_fails_if_votes_not_equals_shares() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        let buy_asset_amount = 10;
        let slot_id = test_vote_with_active_withdrawal_request_precs(
            &algod,
            &creator,
            &investor,
            &project,
            buy_asset_amount,
        )
        .await?;

        // flow + test

        // votes with count != shares count
        let res = vote_flow(&algod, &investor, &project, slot_id, buy_asset_amount + 1).await;
        assert!(res.is_err());

        // votes with count != shares count
        let res = vote_flow(&algod, &investor, &project, slot_id, buy_asset_amount - 1).await;
        assert!(res.is_err());

        // sanity check: votes with count == shares: succeeds
        let res = vote_flow(&algod, &investor, &project, slot_id, buy_asset_amount).await;
        assert!(res.is_ok());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_vote_fails_if_0_votes() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        let buy_asset_amount = 10;
        let slot_id = test_vote_with_active_withdrawal_request_precs(
            &algod,
            &creator,
            &investor,
            &project,
            buy_asset_amount,
        )
        .await?;

        // double check that votes is default value / there are no votes
        let slot_app = algod.application_information(slot_id).await?;
        let initial_vote_count = votes_global_state(&slot_app);
        assert!(initial_vote_count.is_none());

        // flow + test

        let res = vote_flow(&algod, &investor, &project, slot_id, 0).await;
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_vote_fails_if_no_active_withdrawal_request() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        let buy_asset_amount = 10;
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // double check that there's no active withdrawal request
        let slot_app = algod.application_information(slot_id).await?;
        let initial_vote_count = withdrawal_amount_global_state(&slot_app);
        assert!(initial_vote_count.is_none());

        // flow + test

        let res = vote_flow(&algod, &investor, &project, slot_id, buy_asset_amount).await;
        assert!(res.is_err());

        // sanity check: if we submit a withdrawal request, voting succeeds

        // init a withdrawal request
        let init_withdrawal_tx_id =
            init_withdrawal_flow(&algod, &creator, MicroAlgos(123), slot_id).await?;
        let _ = wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

        let res = vote_flow(&algod, &investor, &project, slot_id, buy_asset_amount).await;
        assert!(res.is_ok());

        Ok(())
    }

    /// for convenience, we return a slot id.
    /// This is an arbitrary withdrawal slot we use to perform (one-slot) voting tests.
    async fn test_vote_with_active_withdrawal_request_precs(
        algod: &Algod,
        creator: &Account,
        investor: &Account,
        project: &Project,
        buy_asset_amount: u64,
    ) -> Result<u64> {
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        assert!(!project.withdrawal_slot_ids.is_empty()); // sanity test
        let slot_id = project.withdrawal_slot_ids[0];

        // init a withdrawal request
        let init_withdrawal_tx_id =
            init_withdrawal_flow(&algod, &creator, MicroAlgos(123), slot_id).await?;
        let _ = wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

        // double check that votes is default value / there are no votes
        let slot_app = algod.application_information(slot_id).await?;
        let initial_vote_count = votes_global_state(&slot_app);
        assert!(initial_vote_count.is_none());

        Ok(slot_id)
    }
}
