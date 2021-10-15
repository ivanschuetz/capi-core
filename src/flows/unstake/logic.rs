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
        core::{Address, MicroAlgos},
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        app_state_util::{app_local_state, app_local_state_or_err},
        dependencies,
        flows::reclaim_vote::logic::FIXED_FEE,
        network_util::wait_for_pending_transaction,
        testing::{
            flow::{
                create_project::create_project_flow, invest_in_project::invests_flow,
                unstake::unstake_flow,
            },
            network_test_util::reset_network,
            project_general::{
                check_investor_central_app_local_state,
                test_withdrawal_slot_local_state_initialized_correctly,
            },
            test_data::{creator, investor1, project_specs},
        },
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
        // TODO test that possible votes are removed from withdrawal slots global state

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
