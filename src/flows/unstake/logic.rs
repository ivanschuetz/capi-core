use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, builder::CloseApplication, tx_group::TxGroup, Pay,
        SignedTransaction, Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

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

    // Pay for the shares transfer tx
    let mut pay_shares_xfer_fee_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params
        },
        Pay::new(investor, staking_escrow.address, FIXED_FEE).build(),
    )
    .build();

    let txs_for_group = vec![
        &mut central_app_optout_tx,
        &mut shares_xfer_tx,
        &mut pay_shares_xfer_fee_tx,
    ];
    TxGroup::assign_group_id(txs_for_group)?;

    let signed_shares_xfer_tx = staking_escrow.sign(&shares_xfer_tx, vec![])?;

    Ok(UnstakeToSign {
        central_app_optout_tx,
        shares_xfer_tx: signed_shares_xfer_tx,
        pay_shares_xfer_fee_tx,
    })
}

pub async fn submit_unstake(algod: &Algod, signed: UnstakeSigned) -> Result<String> {
    // crate::debug_msg_pack_submit_par::log_to_msg_pack(&signed);

    let txs = vec![
        signed.central_app_optout_tx,
        signed.shares_xfer_tx_signed,
        signed.pay_shares_xfer_fee_tx,
    ];

    // crate::teal::debug_teal_rendered(&txs, "staking_escrow").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    println!("Unstake tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstakeToSign {
    pub central_app_optout_tx: Transaction,
    pub shares_xfer_tx: SignedTransaction,
    pub pay_shares_xfer_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnstakeSigned {
    pub central_app_optout_tx: SignedTransaction,
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
        flows::unstake::logic::FIXED_FEE,
        network_util::wait_for_pending_transaction,
        state::central_app_state::central_investor_state_from_acc,
        testing::{
            flow::{
                create_project::create_project_flow,
                invest_in_project::{invests_flow, invests_optins_flow},
                unstake::unstake_flow,
            },
            network_test_util::reset_network,
            test_data::{creator, investor1, project_specs},
            TESTS_DEFAULT_PRECISION,
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

        let project =
            create_project_flow(&algod, &creator, &project_specs(), TESTS_DEFAULT_PRECISION)
                .await?;

        invests_optins_flow(&algod, &investor, &project).await?;
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;
        // TODO double check tests for state (at least important) tested (e.g. investor has shares, staking doesn't etc.)

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // opted in to shares
        assert_eq!(0, investor_assets[0].amount); // doesn't have shares (they're sent directly to staking escrow)

        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.central_app_id)?;
        // double check investor's local state
        // shares set to bought asset amount
        assert_eq!(buy_asset_amount, investor_state.shares);
        //  harvested total is 0 (hasn't harvested yet)
        assert_eq!(MicroAlgos(0), investor_state.harvested);

        // double check staking escrow's assets
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len()); // opted in to shares
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);

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
        assert_eq!(1, staking_escrow_assets.len()); // still opted in to shares
        assert_eq!(0, staking_escrow_assets[0].amount); // lost shares

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

        let project =
            create_project_flow(&algod, &creator, &project_specs(), TESTS_DEFAULT_PRECISION)
                .await?;

        invests_optins_flow(&algod, &investor, &project).await?;
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // double check investor's assets
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // opted in to shares
        assert_eq!(0, investor_assets[0].amount); // doesn't have shares (they're sent directly to staking escrow)

        // double check investor's local state
        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.central_app_id)?;
        // shares set to bought asset amount
        assert_eq!(buy_asset_amount, investor_state.shares);
        // harvested total is 0 (hasn't harvested yet)
        assert_eq!(MicroAlgos(0), investor_state.harvested);

        // double check staking escrow's assets
        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len()); // opted in to shares
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);

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
        assert_eq!(1, staking_escrow_assets.len()); // still opted in to shares
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount); // lost shares

        // investor didn't get anything
        let investor_infos = algod.account_information(&investor.address()).await?;
        let investor_assets = &investor_infos.assets;
        assert_eq!(1, investor_assets.len());
        assert_eq!(0, investor_assets[0].amount); // no shares

        let investor_state =
            central_investor_state_from_acc(&investor_infos, project.central_app_id)?;
        // investor local state not changed
        // shares set to bought asset amount
        assert_eq!(buy_asset_amount, investor_state.shares);
        // harvested total is 0 (hasn't harvested yet)
        assert_eq!(MicroAlgos(0), investor_state.harvested);

        // investor didn't pay fees (unstake txs failed)
        assert_eq!(investor_balance_before_unstaking, investor_infos.amount);

        Ok(())
    }
}
