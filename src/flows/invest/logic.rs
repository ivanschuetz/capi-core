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

    let central_app_investor_setup_tx =
        &mut central_app_investor_setup_tx(&params, central_app_id, shares_asset_id, *investor)?;

    // TODO why is this sending the algos to the invest escrow instead of to the central? why not caught by tests yet?
    // should be most likely the central as that's where we withdraw funds from
    let send_algos_tx = &mut TxnBuilder::with(
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
    let pay_escrow_fee_tx = &mut TxnBuilder::with(
        params.clone(),
        Pay::new(*investor, project.invest_escrow.address, FIXED_FEE * 2).build(), // shares xfer + votes xfer txs
    )
    .build();

    let shares_optin_tx = &mut TxnBuilder::with(
        params.clone(),
        AcceptAsset::new(*investor, project.shares_asset_id).build(),
    )
    .build();

    let receive_shares_asset_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
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

    let receive_voting_asset_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params
        },
        TransferAsset::new(
            project.invest_escrow.address,
            project.votes_asset_id,
            asset_count,
            staking_escrow.address,
        )
        .build(),
    )
    .build();

    // Important: order has to be the same as when signing (can we ensure this somehow? vec? tx group type?)
    TxGroup::assign_group_id(vec![
        central_app_investor_setup_tx,
        send_algos_tx,
        shares_optin_tx,
        receive_shares_asset_tx,
        receive_voting_asset_tx,
        pay_escrow_fee_tx,
    ])?;

    let receive_shares_asset_signed_tx = project
        .invest_escrow
        .sign(receive_shares_asset_tx, vec![])?;
    let receive_votes_asset_signed_tx = project
        .invest_escrow
        .sign(receive_voting_asset_tx, vec![])?;

    Ok(InvestToSign {
        project: project.to_owned(),
        central_app_opt_in_tx: central_app_investor_setup_tx.to_owned(),
        payment_tx: send_algos_tx.clone(),
        shares_asset_optin_tx: shares_optin_tx.clone(),
        pay_escrow_fee_tx: pay_escrow_fee_tx.clone(),
        shares_xfer_tx: receive_shares_asset_signed_tx,
        votes_xfer_tx: receive_votes_asset_signed_tx,
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
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.central_app_opt_in_tx.clone(),
    //         signed.payment_tx.clone(),
    //         signed.shares_asset_optin_tx.clone(),
    //         signed.shares_xfer_tx.clone(),
    //         signed.votes_xfer_tx.clone(),
    //         signed.pay_escrow_fee_tx.clone(),
    //     ],
    //     "app_central_approval",
    // )
    // .unwrap();
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.central_app_opt_in_tx.clone(),
    //         signed.payment_tx.clone(),
    //         signed.shares_asset_optin_tx.clone(),
    //         signed.shares_xfer_tx.clone(),
    //         signed.votes_xfer_tx.clone(),
    //         signed.pay_escrow_fee_tx.clone(),
    //     ],
    //     "investing_escrow",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&vec![
            signed.central_app_opt_in_tx.clone(),
            signed.payment_tx.clone(),
            signed.shares_asset_optin_tx.clone(),
            signed.shares_xfer_tx.clone(),
            signed.votes_xfer_tx.clone(),
            signed.pay_escrow_fee_tx.clone(),
        ])
        .await?;
    Ok(InvestResult {
        tx_id: res.tx_id,
        project: signed.project.clone(),
        central_app_investor_setup_tx: signed.central_app_opt_in_tx.clone(),
        payment_tx: signed.payment_tx.clone(),
        shares_asset_optin_tx: signed.shares_asset_optin_tx.clone(),
        pay_escrow_fee_tx: signed.pay_escrow_fee_tx.clone(),
        shares_xfer_tx: signed.shares_xfer_tx.clone(),
        votes_xfer_tx: signed.votes_xfer_tx.clone(),
    })
}

#[cfg(test)]
mod tests {
    use crate::testing::flow::create_project::create_project_flow;
    use crate::testing::flow::invest_in_project::invests_flow;
    use crate::testing::network_test_util::reset_network;
    use crate::{
        dependencies,
        testing::test_data::creator,
        testing::test_data::{investor1, project_specs},
    };
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

        let project = create_project_flow(&algod, &creator, &specs, 3).await?;

        // flow

        let flow_res = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // staking escrow tests

        let staking_escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        // staking escrow received the shares and votes
        let staking_escrow_assets = staking_escrow_infos.assets;
        assert_eq!(2, staking_escrow_assets.len());
        assert_eq!(buy_asset_amount, staking_escrow_assets[0].amount);
        // votes count == shares count
        assert_eq!(buy_asset_amount, staking_escrow_assets[1].amount);
        // staking escrow doesn't send any transactions so not testing balances (we could "double check" though)

        // investor tests

        let investor_infos = algod.account_information(&investor.address()).await?;
        // double check: investor didn't receive any shares or votes
        let investor_assets = investor_infos.assets;
        assert_eq!(1, investor_assets.len()); // investor never opts in to votes, so only 1 (shares)
        assert_eq!(0, investor_assets[0].amount);
        // investor lost algos and fees
        let payed_amount = specs.asset_price * buy_asset_amount;
        assert_eq!(
            flow_res.investor_initial_amount
                - payed_amount
                - flow_res.central_app_optin_tx.transaction.fee
                - flow_res.invest_res.central_app_investor_setup_tx.transaction.fee
                - flow_res.invest_res.shares_asset_optin_tx.transaction.fee
                - flow_res.invest_res.payment_tx.transaction.fee
                - retrieve_payment_amount_from_tx(&flow_res.invest_res.pay_escrow_fee_tx.transaction)? // paid for the escrow's xfers (shares+votes) fees
                - flow_res.invest_res.pay_escrow_fee_tx.transaction.fee, // the fee to pay for the escrow's xfer fee
            investor_infos.amount
        );

        // invest escrow tests

        let invest_escrow = flow_res.project.invest_escrow;
        let invest_escrow_infos = algod.account_information(&invest_escrow.address).await?;
        let invest_escrow_held_assets = invest_escrow_infos.assets;
        // escrow lost the bought assets (shares and votes)
        assert_eq!(invest_escrow_held_assets.len(), 2);
        assert_eq!(
            invest_escrow_held_assets[0].asset_id,
            flow_res.project.shares_asset_id
        );
        assert_eq!(
            invest_escrow_held_assets[0].amount,
            flow_res.project.specs.shares.count - buy_asset_amount
        );
        assert_eq!(
            invest_escrow_held_assets[1].asset_id,
            flow_res.project.votes_asset_id
        );
        assert_eq!(
            invest_escrow_held_assets[1].amount,
            flow_res.project.specs.shares.count - buy_asset_amount
        );
        // escrow received the payed algos
        // Note that escrow doesn't lose algos: the investor sends a payment to cover the escrow's fees.
        assert_eq!(
            flow_res.escrow_initial_amount + payed_amount,
            invest_escrow_infos.amount
        );

        // TODO test the voting asset transfer

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
