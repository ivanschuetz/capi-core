use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, builder::CallApplication, tx_group::TxGroup, AcceptAsset, Pay,
        Transaction, TransferAsset, TxnBuilder,
    },
};
use anyhow::Result;

use crate::flows::create_project::model::Project;

use super::model::{InvestResult, InvestSigned, InvestToSign};

// TODO no constant
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

/// Requires investor to opt in to the app first,
/// we can't do it here: setting local state errors if during opt-in
#[allow(clippy::too_many_arguments)]
pub async fn invest_txs(
    algod: &Algod,
    project: &Project,
    investor: &Address,
    staking_escrow: &ContractAccount,
    central_app_id: u64,
    shares_asset_id: u64,
    asset_count: u64,
    asset_price: MicroAlgos,
) -> Result<InvestToSign> {
    println!("Investing in project: {:?}", project);

    let params = algod.suggested_transaction_params().await?;

    let mut central_app_investor_setup_tx =
        central_app_investor_setup_tx(&params, central_app_id, shares_asset_id, *investor)?;

    // TODO why is this sending the algos to the invest escrow instead of to the central? why not caught by tests yet?
    // should be most likely the central as that's where we withdraw funds from
    let mut send_algos_tx = TxnBuilder::with(
        params.clone(),
        Pay::new(
            *investor,
            project.invest_escrow.address,
            asset_price * asset_count,
        )
        .build(),
    )
    .build();

    // TODO: review including this payment in send_algos_tx (to not have to pay a new fee? or can the fee here actually be 0, since group?: research)
    // note that a reason to _not_ include it is to show it separately to the user, when signing. It can help with clarity (review).
    let mut pay_escrow_fee_tx = TxnBuilder::with(
        params.clone(),
        Pay::new(*investor, project.invest_escrow.address, FIXED_FEE).build(), // shares xfer
    )
    .build();

    let mut shares_optin_tx = TxnBuilder::with(
        params.clone(),
        AcceptAsset::new(*investor, project.shares_asset_id).build(),
    )
    .build();

    let mut receive_shares_asset_tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params
        },
        TransferAsset::new(
            project.invest_escrow.address,
            project.shares_asset_id,
            asset_count,
            staking_escrow.address,
        )
        .build(),
    )
    .build();

    let txs_for_group = vec![
        &mut central_app_investor_setup_tx,
        &mut send_algos_tx,
        &mut shares_optin_tx,
        &mut receive_shares_asset_tx,
        &mut pay_escrow_fee_tx,
    ];
    TxGroup::assign_group_id(txs_for_group)?;

    let receive_shares_asset_signed_tx = project
        .invest_escrow
        .sign(&receive_shares_asset_tx, vec![])?;

    Ok(InvestToSign {
        project: project.to_owned(),
        central_app_setup_tx: central_app_investor_setup_tx,
        payment_tx: send_algos_tx,
        shares_asset_optin_tx: shares_optin_tx,
        pay_escrow_fee_tx,
        shares_xfer_tx: receive_shares_asset_signed_tx,
    })
}

pub fn central_app_investor_setup_tx(
    params: &SuggestedTransactionParams,
    app_id: u64,
    shares_asset_id: u64,
    investor: Address,
) -> Result<Transaction> {
    let tx = TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(investor, app_id)
            .foreign_assets(vec![shares_asset_id])
            .build(),
    )
    .build();
    Ok(tx)
}

pub async fn submit_invest(algod: &Algod, signed: &InvestSigned) -> Result<InvestResult> {
    let txs = vec![
        signed.central_app_setup_tx.clone(),
        signed.payment_tx.clone(),
        signed.shares_asset_optin_tx.clone(),
        signed.shares_xfer_tx.clone(),
        signed.pay_escrow_fee_tx.clone(),
    ];

    // crate::teal::debug_teal_rendered(&txs, "app_central_approval").unwrap();
    // crate::teal::debug_teal_rendered(&txs, "investing_escrow").unwrap();

    let res = algod.broadcast_signed_transactions(&txs).await?;
    Ok(InvestResult {
        tx_id: res.tx_id,
        project: signed.project.clone(),
        central_app_investor_setup_tx: signed.central_app_setup_tx.clone(),
        payment_tx: signed.payment_tx.clone(),
        shares_asset_optin_tx: signed.shares_asset_optin_tx.clone(),
        pay_escrow_fee_tx: signed.pay_escrow_fee_tx.clone(),
        shares_xfer_tx: signed.shares_xfer_tx.clone(),
    })
}

#[cfg(test)]
mod tests {
    use crate::flows::create_project::model::Project;
    use crate::flows::harvest::logic::calculate_entitled_harvest;
    use crate::network_util::wait_for_pending_transaction;
    use crate::state::central_app_state::{
        central_global_state, central_investor_state, central_investor_state_from_acc,
    };
    use crate::testing::flow::create_project::create_project_flow;
    use crate::testing::flow::customer_payment_and_drain_flow::customer_payment_and_drain_flow;
    use crate::testing::flow::invest_in_project::{invests_flow, invests_optins_flow};
    use crate::testing::flow::stake::stake_flow;
    use crate::testing::flow::unstake::unstake_flow;
    use crate::testing::network_test_util::reset_network;
    use crate::testing::test_data::{customer, investor2};
    use crate::testing::TESTS_DEFAULT_PRECISION;
    use crate::{
        dependencies,
        testing::test_data::creator,
        testing::test_data::{investor1, project_specs},
    };
    use algonaut::algod::v2::Algod;
    use algonaut::transaction::account::Account;
    use algonaut::{
        core::MicroAlgos,
        transaction::{Transaction, TransactionType},
    };
    use anyhow::{anyhow, Result};
    use serial_test::serial;
    use tokio::test;

    #[test]
    #[serial] // reset network (cmd)
    async fn test_invests_flow() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI
        let buy_asset_amount = 10;
        let specs = project_specs();

        let project =
            create_project_flow(&algod, &creator, &specs, TESTS_DEFAULT_PRECISION).await?;

        // precs

        invests_optins_flow(&algod, &investor, &project).await?;

        // flow

        let flow_res = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // staking escrow tests

        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        // staking escrow received the shares
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(1, staking_escrow_assets.len());
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);
        // staking escrow doesn't send any transactions so not testing balances (we could "double check" though)

        // investor tests

        let investor_infos = algod.account_information(&investor.address()).await?;
        let central_investor_state =
            central_investor_state_from_acc(&investor_infos, project.central_app_id)?;

        // investor has shares
        assert_eq!(buy_asset_amount, central_investor_state.shares);

        // double check: investor didn't receive any shares
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len());
        assert_eq!(0, investor_assets[0].amount);

        // investor lost algos and fees
        let paid_amount = specs.asset_price * buy_asset_amount;
        assert_eq!(
            flow_res.investor_initial_amount
                - paid_amount
                - flow_res.invest_res.central_app_investor_setup_tx.transaction.fee
                - flow_res.invest_res.shares_asset_optin_tx.transaction.fee
                - flow_res.invest_res.payment_tx.transaction.fee
                - retrieve_payment_amount_from_tx(&flow_res.invest_res.pay_escrow_fee_tx.transaction)? // paid for the escrow's xfers (shares) fees
                - flow_res.invest_res.pay_escrow_fee_tx.transaction.fee, // the fee to pay for the escrow's xfer fee
            investor_infos.amount
        );

        // invest escrow tests

        let invest_escrow = flow_res.project.invest_escrow;
        let invest_escrow_infos = algod.account_information(&invest_escrow.address).await?;
        let invest_escrow_held_assets = invest_escrow_infos.assets;
        // escrow lost the bought assets
        assert_eq!(invest_escrow_held_assets.len(), 1);
        assert_eq!(
            invest_escrow_held_assets[0].asset_id,
            flow_res.project.shares_asset_id
        );
        assert_eq!(
            invest_escrow_held_assets[0].amount,
            flow_res.project.specs.shares.count - buy_asset_amount
        );
        // escrow received the payed algos
        // Note that escrow doesn't lose algos: the investor sends a payment to cover the escrow's fees.
        assert_eq!(
            flow_res.escrow_initial_amount + paid_amount,
            invest_escrow_infos.amount
        );

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_investing_twice() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI
        let buy_asset_amount = 10;
        let buy_asset_amount2 = 20;
        let specs = project_specs();

        let project =
            create_project_flow(&algod, &creator, &specs, TESTS_DEFAULT_PRECISION).await?;

        // precs

        invests_optins_flow(&algod, &investor, &project).await?;

        // flow

        invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // double check: investor has shares for first investment
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        assert_eq!(buy_asset_amount, investor_state.shares);

        invests_flow(&algod, &investor, buy_asset_amount2, &project).await?;

        // tests

        // investor has shares for both investments
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        assert_eq!(buy_asset_amount + buy_asset_amount2, investor_state.shares);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_investing_and_staking() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI
        let stake_amount = 10;
        let invest_amount = 20;
        let specs = project_specs();

        let project =
            create_project_flow(&algod, &creator, &specs, TESTS_DEFAULT_PRECISION).await?;

        // precs

        invests_optins_flow(&algod, &investor, &project).await?;

        // for user to have some free shares (assets) to stake
        buy_and_unstake_shares(&algod, &investor, &project, stake_amount).await?;

        // flow

        // buy shares: automatically staked
        invests_optins_flow(&algod, &investor, &project).await?; // optin again: unstaking opts user out
        invests_flow(&algod, &investor, invest_amount, &project).await?;

        // double check: investor has shares for first investment
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        assert_eq!(invest_amount, investor_state.shares);

        // stake shares
        stake_flow(&algod, &project, &investor, stake_amount).await?;

        // tests

        // investor has shares for investment + staking
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        assert_eq!(stake_amount + invest_amount, investor_state.shares);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_staking_and_investing() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI
        let stake_amount = 10;
        let invest_amount = 20;
        let specs = project_specs();

        let project =
            create_project_flow(&algod, &creator, &specs, TESTS_DEFAULT_PRECISION).await?;

        // precs

        invests_optins_flow(&algod, &investor, &project).await?;

        // for user to have some free shares (assets) to stake
        buy_and_unstake_shares(&algod, &investor, &project, stake_amount).await?;

        // flow

        // stake shares
        invests_optins_flow(&algod, &investor, &project).await?; // optin again: unstaking opts user out
        stake_flow(&algod, &project, &investor, stake_amount).await?;

        // double check: investor has staked shares
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        assert_eq!(stake_amount, investor_state.shares);

        // buy shares: automatically staked
        invests_flow(&algod, &investor, invest_amount, &project).await?;

        // tests

        // investor has shares for investment + staking
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        assert_eq!(stake_amount + invest_amount, investor_state.shares);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_increments_shares_when_staking_twice() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();

        // UI
        let stake_amount1 = 10;
        let stake_amount2 = 20;
        // an amount we unstake and will not stake again, to make the test a little more robust
        let invest_amount_not_stake = 5;
        let specs = project_specs();

        let project =
            create_project_flow(&algod, &creator, &specs, TESTS_DEFAULT_PRECISION).await?;

        // precs

        invests_optins_flow(&algod, &investor, &project).await?;

        // for user to have free shares (assets) to stake
        buy_and_unstake_shares(
            &algod,
            &investor,
            &project,
            stake_amount1 + stake_amount2 + invest_amount_not_stake,
        )
        .await?;

        // flow

        // stake shares
        invests_optins_flow(&algod, &investor, &project).await?; // optin again: unstaking opts user out
        stake_flow(&algod, &project, &investor, stake_amount1).await?;

        // double check: investor has staked shares
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        assert_eq!(stake_amount1, investor_state.shares);

        // stake more shares
        stake_flow(&algod, &project, &investor, stake_amount2).await?;

        // tests

        // investor has shares for investment + staking
        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        assert_eq!(stake_amount1 + stake_amount2, investor_state.shares);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_invest_after_drain_inits_already_harvested_correctly() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();
        let drainer = investor2();
        let customer = customer();

        // UI
        let buy_asset_amount = 10;
        let specs = project_specs();

        let project =
            create_project_flow(&algod, &creator, &specs, TESTS_DEFAULT_PRECISION).await?;

        // precs

        // add some funds
        let central_funds = MicroAlgos(10 * 1_000_000);
        customer_payment_and_drain_flow(&algod, &drainer, &customer, central_funds, &project)
            .await?;

        invests_optins_flow(&algod, &investor, &project).await?;

        // flow
        invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // tests

        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        let central_state = central_global_state(&algod, project.central_app_id).await?;

        let investor_entitled_harvest = calculate_entitled_harvest(
            central_state.received,
            project.specs.shares.count,
            buy_asset_amount,
            TESTS_DEFAULT_PRECISION,
            project.specs.investors_share,
        );

        // investing inits the "harvested" amount to entitled amount (to prevent double harvest)
        assert_eq!(investor_entitled_harvest, investor_state.harvested);

        Ok(())
    }

    #[test]
    #[serial] // reset network (cmd)
    async fn test_stake_after_drain_inits_already_harvested_correctly() -> Result<()> {
        reset_network()?;

        // deps
        let algod = dependencies::algod();
        let creator = creator();
        let investor = investor1();
        let drainer = investor2();
        let customer = customer();

        // UI
        let buy_asset_amount = 10;
        let specs = project_specs();

        let project =
            create_project_flow(&algod, &creator, &specs, TESTS_DEFAULT_PRECISION).await?;

        // precs

        // add some funds
        let central_funds = MicroAlgos(10 * 1_000_000);
        customer_payment_and_drain_flow(&algod, &drainer, &customer, central_funds, &project)
            .await?;

        invests_optins_flow(&algod, &investor, &project).await?;

        // for user to have some free shares (assets) to stake
        buy_and_unstake_shares(&algod, &investor, &project, buy_asset_amount).await?;

        // flow
        invests_optins_flow(&algod, &investor, &project).await?; // optin again: unstaking opts user out
        stake_flow(&algod, &project, &investor, buy_asset_amount).await?;

        // tests

        let investor_state =
            central_investor_state(&algod, &investor.address(), project.central_app_id).await?;
        let central_state = central_global_state(&algod, project.central_app_id).await?;

        let investor_entitled_harvest = calculate_entitled_harvest(
            central_state.received,
            project.specs.shares.count,
            buy_asset_amount,
            TESTS_DEFAULT_PRECISION,
            project.specs.investors_share,
        );

        // staking inits the "harvested" amount to entitled amount (to prevent double harvest)
        assert_eq!(investor_entitled_harvest, investor_state.harvested);

        Ok(())
    }

    async fn buy_and_unstake_shares(
        algod: &Algod,
        investor: &Account,
        project: &Project,
        shares_amount: u64,
    ) -> Result<()> {
        invests_flow(&algod, &investor, shares_amount, &project).await?;
        let unstake_tx_id = unstake_flow(&algod, &project, &investor, shares_amount).await?;
        wait_for_pending_transaction(&algod, &unstake_tx_id).await?;
        Ok(())
    }

    // TODO refactor with fn in other test (same name)
    fn retrieve_payment_amount_from_tx(tx: &Transaction) -> Result<MicroAlgos> {
        match &tx.txn_type {
            TransactionType::Payment(p) => Ok(p.amount),
            _ => Err(anyhow!(
                "Invalid state: tx is expected to be a payment tx: {:?}",
                tx
            )),
        }
    }
}
