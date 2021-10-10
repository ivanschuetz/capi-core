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
    staking_escrow: &ContractAccount,
) -> Result<UnstakeToSign> {
    let params = algod.suggested_transaction_params().await?;

    // App call to validate the retrieved shares count and clear local state
    let app_call_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CloseApplication::new(investor, central_app_id).build(),
    )
    .build();

    // Retrieve investor's assets from staking escrow
    let shares_xfer_tx = &mut TxnBuilder::with(
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
    let pay_shares_xfer_fee_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(investor, staking_escrow.address, FIXED_FEE).build(),
    )
    .build();

    TxGroup::assign_group_id(vec![app_call_tx, shares_xfer_tx, pay_shares_xfer_fee_tx])?;

    let signed_shares_xfer_tx = staking_escrow.sign(&shares_xfer_tx, vec![])?;

    Ok(UnstakeToSign {
        app_call_tx: app_call_tx.clone(),
        shares_xfer_tx: signed_shares_xfer_tx.clone(),
        pay_shares_xfer_fee_tx: pay_shares_xfer_fee_tx.clone(),
    })
}

pub async fn submit_unstake(algod: &Algod, signed: UnstakeSigned) -> Result<String> {
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx.clone(),
    //         signed.shares_xfer_tx_signed.clone(),
    //         signed.pay_shares_xfer_fee_tx.clone(),
    //     ],
    //     "staking_escrow",
    // )
    // .unwrap();
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.app_call_tx.clone(),
    //         signed.shares_xfer_tx_signed.clone(),
    //         signed.pay_shares_xfer_fee_tx.clone(),
    //     ],
    //     "app_central_approval",
    // )
    // .unwrap();
    let res = algod
        .broadcast_signed_transactions(&[
            signed.app_call_tx,
            signed.shares_xfer_tx_signed,
            signed.pay_shares_xfer_fee_tx,
        ])
        .await?;
    println!("Unstake tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstakeToSign {
    pub app_call_tx: Transaction,
    pub shares_xfer_tx: SignedTransaction,
    pub pay_shares_xfer_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstakeSigned {
    pub app_call_tx: SignedTransaction,
    pub shares_xfer_tx_signed: SignedTransaction,
    pub pay_shares_xfer_fee_tx: SignedTransaction,
}

#[cfg(test)]
mod tests {
    use algonaut::core::MicroAlgos;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::reclaim_vote::logic::FIXED_FEE,
        network_util::wait_for_pending_transaction,
        testing::{
            flow::{create_project::create_project_flow, invest_in_project::invests_flow},
            network_test_util::reset_network,
            project_general::check_investor_local_state,
            test_data::{creator, investor1, project_specs},
        },
    };

    use super::{submit_unstake, unstake, UnstakeSigned};

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

        let project = create_project_flow(&algod, &creator, &project_specs()).await?;

        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;
        // TODO double check tests for state (at least important) tested (e.g. investor has shares, staking doesn't etc.)

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // opted in to shares
        assert_eq!(0, investor_assets[0].amount); // doesn't have shares (they're sent directly to staking escrow)

        // double check investor's local state
        check_investor_local_state(
            investor_infos.apps_local_state,
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

        let unstake_to_sign = unstake(
            &algod,
            investor.address(),
            unstake_share_amount,
            project.shares_asset_id,
            project.central_app_id,
            &project.staking_escrow,
        )
        .await?;

        let signed_app_call_tx = investor.sign_transaction(&unstake_to_sign.app_call_tx)?;
        let signed_pay_shares_xfer_fee_tx =
            investor.sign_transaction(&unstake_to_sign.pay_shares_xfer_fee_tx)?;

        let tx_id = submit_unstake(
            &algod,
            UnstakeSigned {
                app_call_tx: signed_app_call_tx,
                shares_xfer_tx_signed: unstake_to_sign.shares_xfer_tx,
                pay_shares_xfer_fee_tx: signed_pay_shares_xfer_fee_tx,
            },
        )
        .await?;

        let _ = wait_for_pending_transaction(&algod, &tx_id).await?;

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

        // investor paid the fees (app call + xfer + xfer fee)
        assert_eq!(
            investor_balance_before_unstaking - FIXED_FEE * 3,
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

        let project = create_project_flow(&algod, &creator, &project_specs()).await?;

        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // opted in to shares
        assert_eq!(0, investor_assets[0].amount); // doesn't have shares (they're sent directly to staking escrow)

        // double check investor's local state
        check_investor_local_state(
            investor_infos.apps_local_state,
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

        let unstake_to_sign = unstake(
            &algod,
            investor.address(),
            unstake_share_amount,
            project.shares_asset_id,
            project.central_app_id,
            &project.staking_escrow,
        )
        .await?;

        let signed_app_call_tx = investor.sign_transaction(&unstake_to_sign.app_call_tx)?;
        let signed_pay_shares_xfer_fee_tx =
            investor.sign_transaction(&unstake_to_sign.pay_shares_xfer_fee_tx)?;

        let tx_id_res = submit_unstake(
            &algod,
            UnstakeSigned {
                app_call_tx: signed_app_call_tx,
                shares_xfer_tx_signed: unstake_to_sign.shares_xfer_tx,
                pay_shares_xfer_fee_tx: signed_pay_shares_xfer_fee_tx,
            },
        )
        .await;

        assert!(tx_id_res.is_err());

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

        // investor local state not changed
        check_investor_local_state(
            investor_infos.apps_local_state,
            project.central_app_id,
            // shares set to bought asset amount
            buy_asset_amount,
            // harvested total is 0 (hasn't harvested yet)
            MicroAlgos(0),
        );

        // investor didn't pay fees (unstake txs failed)
        assert_eq!(investor_balance_before_unstaking, investor_infos.amount);

        Ok(())
    }
}
