use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, builder::CloseApplication, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn unstake(
    algod: &Algod,
    investor: Address,
    // required to be === held shares (otherwise central app rejects the tx)
    share_count: u64,
    shares_asset_id: u64,
    central_app_id: u64,
    wthdrawal_slot_ids: &Vec<u64>,
    staking_escrow: &ContractAccount,
) -> Result<UnstakeToSign> {
    let params = algod.suggested_transaction_params().await?;

    // App call to validate the retrieved shares count and clear local state
    let mut central_app_optout_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CloseApplication::new(investor, central_app_id).build(),
    )
    .build();

    // Retrieve investor's assets from staking escrow
    let mut shares_xfer_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        TransferAsset::new(
            staking_escrow.address,
            shares_asset_id,
            share_count,
            investor,
        )
        .build(),
    )
    .build();

    // Pay for the vote tokens transfer tx
    let mut pay_shares_xfer_fee_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(investor, staking_escrow.address, FIXED_FEE).build(),
    )
    .build();

    // Clear withdrawal slots local state, removing also possible votes from global state
    let mut slot_optout_txs = vec![];
    for slot_id in wthdrawal_slot_ids {
        slot_optout_txs.push(withdrawal_slot_optout_tx(&params, &investor, *slot_id));
    }

    let mut txs_for_group = vec![
        &mut central_app_optout_tx,
        &mut shares_xfer_tx,
        &mut pay_shares_xfer_fee_tx,
    ];
    txs_for_group.extend(slot_optout_txs.iter_mut().collect::<Vec<_>>());
    TxGroup::assign_group_id(txs_for_group)?;

    let signed_shares_xfer_tx = staking_escrow.sign(&shares_xfer_tx, vec![])?;

    Ok(UnstakeToSign {
        central_app_optout_tx,
        slot_optout_txs,
        shares_xfer_tx: signed_shares_xfer_tx,
        pay_shares_xfer_fee_tx,
    })
}

fn withdrawal_slot_optout_tx(
    params: &SuggestedTransactionParams,
    investor: &Address,
    slot_id: u64,
) -> Transaction {
    TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CloseApplication::new(*investor, slot_id).build(),
    )
    .build()
}

pub async fn submit_unstake(algod: &Algod, signed: UnstakeSigned) -> Result<String> {
    let mut txs = vec![
        signed.central_app_optout_tx,
        signed.shares_xfer_tx_signed,
        signed.pay_shares_xfer_fee_tx,
    ];
    txs.extend(signed.slot_optout_txs.clone());

    // crate::teal::debug_teal_rendered(&txs, "staking_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "withdrawal_slot_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    println!("Unstake tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstakeToSign {
    pub central_app_optout_tx: Transaction,
    pub slot_optout_txs: Vec<Transaction>,
    pub shares_xfer_tx: SignedTransaction,
    pub pay_shares_xfer_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstakeSigned {
    pub central_app_optout_tx: SignedTransaction,
    pub slot_optout_txs: Vec<SignedTransaction>,
    pub shares_xfer_tx_signed: SignedTransaction,
    pub pay_shares_xfer_fee_tx: SignedTransaction,
}

#[cfg(test)]
mod tests {
    use algonaut::{
        algod::v2::Algod,
        core::{Address, MicroAlgos, SuggestedTransactionParams},
        transaction::{builder::ClearApplication, Transaction, TxnBuilder},
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        app_state_util::{app_local_state, app_local_state_or_err},
        dependencies,
        flows::{
            reclaim_vote::logic::FIXED_FEE,
            unstake::logic::{submit_unstake, unstake, UnstakeSigned},
            vote::logic::{submit_vote, vote, VoteSigned},
        },
        network_util::wait_for_pending_transaction,
        testing::{
            flow::{
                create_project::create_project_flow, init_withdrawal::init_withdrawal_flow,
                invest_in_project::invests_flow, unstake::unstake_flow, vote::vote_flow,
            },
            network_test_util::reset_network,
            project_general::{
                check_investor_central_app_local_state,
                test_withdrawal_slot_local_state_initialized_correctly,
            },
            test_data::{creator, investor1, investor2, project_specs},
        },
        withdrawal_app_state::{valid_local_state, votes_global_state, votes_local_state},
    };

    #[test]
    #[serial]
    async fn test_unstake() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI

        let buy_asset_amount = 10;

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;

        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;
        // TODO double check tests for state (at least important) tested (e.g. investor has shares, staking doesn't etc.)

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // opted in to shares
        assert_eq!(0, investor_assets[0].amount); // doesn't have shares (they're sent directly to staking escrow)

        let central_app_local_state =
            app_local_state_or_err(&investor_infos.apps_local_state, project.central_app_id)?;

        // double check investor's local state
        check_investor_central_app_local_state(
            central_app_local_state,
            project.central_app_id,
            // shares set to bought asset amount
            buy_asset_amount,
            //  harvested total is 0 (hasn't harvested yet)
            MicroAlgos(0),
        );

        // double check staking escrow's assets
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(2, staking_escrow_assets.len()); // opted in to shares and votes
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);
        assert_eq!(buy_asset_amount, staking_escrow_assets[1].amount);

        // remember state
        let investor_balance_before_unstaking = investor_infos.amount;

        // flow

        // in the real application, unstake_share_amount is retrieved from indexer
        let unstake_share_amount = buy_asset_amount;

        let unstake_tx_id = unstake_flow(&algod, &project, &investor, unstake_share_amount).await?;
        println!("?? unstake tx id: {:?}", unstake_tx_id);
        let _ = wait_for_pending_transaction(&algod, &unstake_tx_id).await?;

        // shares not anymore in staking escrow
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(2, staking_escrow_assets.len()); // still opted in to shares and votes
        assert_eq!(0, staking_escrow_assets[0].amount); // lost shares
        assert_eq!(buy_asset_amount, staking_escrow_assets[1].amount); // still has votes

        // investor got shares
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len());
        assert_eq!(buy_asset_amount, investor_assets[0].amount); // got the shares

        // investor local state cleared (opted out)
        assert_eq!(0, investor_infos.apps_local_state.len());

        // withdrawal slots local state cleared (opted out)
        for slot_id in &project.withdrawal_slot_ids {
            test_withdrawal_slot_local_state_cleared(&algod, &investor.address(), *slot_id).await?;
        }

        // investor paid the fees (app call + xfer + xfer fee + n slots)
        assert_eq!(
            investor_balance_before_unstaking
                - FIXED_FEE * 3
                - FIXED_FEE * project.withdrawal_slot_ids.len() as u64,
            investor_infos.amount
        );

        Ok(())
    }

    // TODO think how to implement partial unstaking: it should be common that investors want to sell only a part of their shares
    // currently we require opt-out to prevent double harvest, REVIEW
    #[test]
    #[serial]
    async fn test_partial_unstake_not_allowed() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI

        let partial_amount = 2;
        let buy_asset_amount = partial_amount + 8;

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;

        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // opted in to shares
        assert_eq!(0, investor_assets[0].amount); // doesn't have shares (they're sent directly to staking escrow)

        let central_app_local_state =
            app_local_state_or_err(&investor_infos.apps_local_state, project.central_app_id)?;

        // double check investor's local state
        check_investor_central_app_local_state(
            central_app_local_state,
            project.central_app_id,
            // shares set to bought asset amount
            buy_asset_amount,
            // harvested total is 0 (hasn't harvested yet)
            MicroAlgos(0),
        );

        // double check staking escrow's assets
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(2, staking_escrow_assets.len()); // opted in to shares and votes
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);
        assert_eq!(buy_asset_amount, staking_escrow_assets[1].amount);

        // remember state
        let investor_balance_before_unstaking = investor_infos.amount;

        // flow

        let unstake_share_amount = partial_amount;

        let unstake_result = unstake_flow(&algod, &project, &investor, unstake_share_amount).await;

        assert!(unstake_result.is_err());

        // shares still in staking escrow
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(2, staking_escrow_assets.len()); // still opted in to shares and votes
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount); // lost shares
        assert_eq!(buy_asset_amount, staking_escrow_assets[1].amount); // still has votes

        // investor didn't get anything
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len());
        assert_eq!(0, investor_assets[0].amount); // no shares

        let central_app_local_state =
            app_local_state_or_err(&investor_infos.apps_local_state, project.central_app_id)?;

        // investor local state not changed
        check_investor_central_app_local_state(
            central_app_local_state,
            project.central_app_id,
            // shares set to bought asset amount
            buy_asset_amount,
            // harvested total is 0 (hasn't harvested yet)
            MicroAlgos(0),
        );

        // local state in withdrawal slots not changed
        for slot_id in project.withdrawal_slot_ids {
            // we repurpose the initial state test, as before opting out we don't do anything with the withdrawal slots here,
            // so unchanged == initialized state
            test_withdrawal_slot_local_state_initialized_correctly(
                &algod,
                &investor.address(),
                slot_id,
            )
            .await?;
        }

        // investor didn't pay fees (unstake txs failed)
        assert_eq!(investor_balance_before_unstaking, investor_infos.amount);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_unstake_removes_possible_withdrawal_request_votes() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();
        let additional_voter = investor2();

        // UI

        let buy_asset_amount = 10;
        let buy_asset_amount_additional_voter = 4;

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;

        // invest: needed to be able to vote
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;
        let _ = invests_flow(
            &algod,
            &additional_voter,
            buy_asset_amount_additional_voter,
            &project,
        )
        .await?;

        // select 2 arbitrary slots
        assert!(project.withdrawal_slot_ids.len() > 1); // just a preliminary check for index access
        let slot_id1 = project.withdrawal_slot_ids[0];
        let slot_id2 = project.withdrawal_slot_ids[1];

        // init 2 withdrawal requests
        let init_withdrawal_tx_id1 =
            init_withdrawal_flow(&algod, &creator, MicroAlgos(123), slot_id1).await?;
        let _ = wait_for_pending_transaction(&algod, &init_withdrawal_tx_id1).await?;
        let init_withdrawal_tx_id2 =
            init_withdrawal_flow(&algod, &creator, MicroAlgos(23456789), slot_id2).await?;
        let _ = wait_for_pending_transaction(&algod, &init_withdrawal_tx_id2).await?;

        // vote for withdrawal requests
        let vote_tx_id1 =
            vote_flow(&algod, &investor, &project, slot_id1, buy_asset_amount).await?;
        wait_for_pending_transaction(&algod, &vote_tx_id1).await?;
        let vote_tx_id2 =
            vote_flow(&algod, &investor, &project, slot_id2, buy_asset_amount).await?;
        wait_for_pending_transaction(&algod, &vote_tx_id2).await?;
        // another investor votes: just to check that when unstaking we're actually substracting the votes instead of possibly just resetting to 0

        let vote_tx_id3 = vote_flow(
            &algod,
            &additional_voter,
            &project,
            slot_id2,
            buy_asset_amount_additional_voter,
        )
        .await?;
        wait_for_pending_transaction(&algod, &vote_tx_id3).await?;

        // flow

        // investor unstakes shares
        let unstake_tx_id = unstake_flow(&algod, &project, &investor, buy_asset_amount).await?;
        let _ = wait_for_pending_transaction(&algod, &unstake_tx_id).await?;

        // test

        // check that votes were removed from slot1: only this investor voted, so 0 votes now
        let slot1 = algod.application_information(slot_id1).await?;
        let slot1_votes_global_state = votes_global_state(&slot1);
        assert!(slot1_votes_global_state.is_some());
        assert_eq!(0, slot1_votes_global_state.unwrap());

        // check that votes were removed from slot2: another investor voted too, so those votes remain
        let slot2 = algod.application_information(slot_id2).await?;
        let slot2_votes_global_state = votes_global_state(&slot2);
        assert!(slot2_votes_global_state.is_some());
        assert_eq!(
            buy_asset_amount_additional_voter,
            slot2_votes_global_state.unwrap()
        );

        // double check that shares not anymore in staking escrow
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(2, staking_escrow_assets.len()); // still opted in to shares and votes
        assert_eq!(
            buy_asset_amount_additional_voter,
            staking_escrow_assets[0].amount
        ); // lost unstaked shares
        assert_eq!(
            buy_asset_amount + buy_asset_amount_additional_voter,
            staking_escrow_assets[1].amount
        ); // still has votes TODO remove vote tokens (everywhere)

        // double check that investor got shares
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len());
        assert_eq!(buy_asset_amount, investor_assets[0].amount); // got the shares

        // double check that investor local state was cleared (opted out)
        assert_eq!(0, investor_infos.apps_local_state.len());

        // double check that withdrawal slots local state cleared (opted out)
        for slot_id in &project.withdrawal_slot_ids {
            test_withdrawal_slot_local_state_cleared(&algod, &investor.address(), *slot_id).await?;
        }

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cant_unstake_if_local_state_cleared() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI

        let buy_asset_amount = 10;

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // select a slot
        assert!(!project.withdrawal_slot_ids.is_empty());
        let slot_id = project.withdrawal_slot_ids[0];

        // flow

        // clear local state of a slot (can be any / multiple)
        // note that this is not done via the app, but sending the tx externally (likely maliciously, e.g. to be able to double-vote)
        let params = algod.suggested_transaction_params().await?;
        let clear_state_tx = clear_local_state_tx(&params, &investor.address(), slot_id);
        let signed_clear_state_tx = investor.sign_transaction(&clear_state_tx)?;
        let clear_state_tx_id = algod
            .broadcast_signed_transaction(&signed_clear_state_tx)
            .await?
            .tx_id;
        wait_for_pending_transaction(&algod, &clear_state_tx_id).await?;

        // double check that local state is cleared
        let account = algod.account_information(&investor.address()).await?;
        let local_vote_amount = votes_local_state(&account.apps_local_state, slot_id);
        assert!(local_vote_amount.is_none());
        let local_valid_flag = valid_local_state(&account.apps_local_state, slot_id);
        assert!(local_valid_flag.is_none());

        // try to unstakes shares
        let to_sign = unstake(
            &algod,
            investor.address(),
            buy_asset_amount,
            project.shares_asset_id,
            project.central_app_id,
            &project.withdrawal_slot_ids,
            &project.staking_escrow,
        )
        .await?;
        let signed_central_app_optout =
            investor.sign_transaction(&to_sign.central_app_optout_tx)?;
        let mut signed_slots_setup_txs = vec![];
        for slot_optout_tx in to_sign.slot_optout_txs {
            signed_slots_setup_txs.push(investor.sign_transaction(&slot_optout_tx)?);
        }
        let signed_pay_xfer_fees = investor.sign_transaction(&to_sign.pay_shares_xfer_fee_tx)?;
        let res = submit_unstake(
            &algod,
            UnstakeSigned {
                central_app_optout_tx: signed_central_app_optout,
                slot_optout_txs: signed_slots_setup_txs,
                shares_xfer_tx_signed: to_sign.shares_xfer_tx,
                pay_shares_xfer_fee_tx: signed_pay_xfer_fees,
            },
        )
        .await;

        // test

        // the local state was cleared, with includes the valid flag: the smart contract rejects unstaking
        assert!(res.is_err());

        // double check that shares still in staking escrow
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(2, staking_escrow_assets.len()); // still opted in to shares and votes
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);

        Ok(())
    }

    #[test]
    #[serial]
    async fn test_cant_vote_if_local_state_cleared() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI

        let buy_asset_amount = 10;

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // select a slot
        assert!(!project.withdrawal_slot_ids.is_empty());
        let slot_id = project.withdrawal_slot_ids[0];

        // init withdrawal request (to have something to vote for)
        let init_withdrawal_tx_id =
            init_withdrawal_flow(&algod, &creator, MicroAlgos(123), slot_id).await?;
        let _ = wait_for_pending_transaction(&algod, &init_withdrawal_tx_id).await?;

        // flow

        // clear local state of a slot (can be any / multiple)
        // note that this is not done via the app, but sending the tx externally (likely maliciously, e.g. to be able to double-vote)
        let params = algod.suggested_transaction_params().await?;
        let clear_state_tx = clear_local_state_tx(&params, &investor.address(), slot_id);
        let signed_clear_state_tx = investor.sign_transaction(&clear_state_tx)?;
        let clear_state_tx_id = algod
            .broadcast_signed_transaction(&signed_clear_state_tx)
            .await?
            .tx_id;
        wait_for_pending_transaction(&algod, &clear_state_tx_id).await?;

        // double check that local state is cleared
        let account = algod.account_information(&investor.address()).await?;
        let local_vote_amount = votes_local_state(&account.apps_local_state, slot_id);
        assert!(local_vote_amount.is_none());
        let local_valid_flag = valid_local_state(&account.apps_local_state, slot_id);
        assert!(local_valid_flag.is_none());

        // try to vote
        let vote_to_sign = vote(
            &algod,
            investor.address(),
            project.central_app_id,
            slot_id,
            buy_asset_amount,
        )
        .await?;

        let signed_vote_tx = investor.sign_transaction(&vote_to_sign.vote_tx)?;
        let signed_validate_vote_count_tx =
            investor.sign_transaction(&vote_to_sign.validate_vote_count_tx)?;
        let res = submit_vote(
            &algod,
            &VoteSigned {
                vote_tx: signed_vote_tx,
                validate_vote_count_tx: signed_validate_vote_count_tx,
            },
        )
        .await;

        // test

        // the local state was cleared, with includes the valid flag: the smart contract rejects voting
        assert!(res.is_err());

        Ok(())
    }

    // test-only tx
    fn clear_local_state_tx(
        params: &SuggestedTransactionParams,
        sender: &Address,
        app_id: u64,
    ) -> Transaction {
        TxnBuilder::with(
            SuggestedTransactionParams {
                fee: FIXED_FEE,
                ..params.clone()
            },
            ClearApplication::new(*sender, app_id).build(),
        )
        .build()
    }

    async fn test_withdrawal_slot_local_state_cleared(
        algod: &Algod,
        investor_address: &Address,
        slot_app_id: u64,
    ) -> Result<()> {
        let account = algod.account_information(investor_address).await?;
        let local_state = account.apps_local_state;
        let app_local_state = app_local_state(&local_state, slot_app_id);
        // println!("LVotes base64: {:?}", BASE64.encode(b"LVotes"));
        // println!("Valid base64: {:?}", BASE64.encode(b"Valid"));
        assert!(app_local_state.is_none());
        Ok(())
    }
}
