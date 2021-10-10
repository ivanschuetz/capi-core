use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    model::algod::v2::{Account, AssetHolding},
    transaction::{
        account::ContractAccount, tx_group::TxGroup, Pay, SignedTransaction, Transaction,
        TransferAsset, TxnBuilder,
    },
};
use anyhow::{anyhow, Result};

// TODO no constants
pub const MIN_BALANCE: MicroAlgos = MicroAlgos(100_000);
// TODO confirm this is needed
// see more notes in old repo
pub const FIXED_FEE: MicroAlgos = MicroAlgos(1_000);

pub async fn withdraw(
    algod: &Algod,
    creator: Address,
    amount: MicroAlgos,
    votes_asset_id: u64,
    central_escrow: &ContractAccount,
    votes_in_escrow: &ContractAccount,
    votes_out_escrow: &ContractAccount,
) -> Result<WithdrawToSign> {
    let params = algod.suggested_transaction_params().await?;

    // Escrow call to withdraw the amount
    let withdraw_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(central_escrow.address, creator, amount).build(),
    )
    .build();

    // The creator pays the fee of the withdraw tx (signed by central escrow)
    let pay_withdraw_fee_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(creator, central_escrow.address, FIXED_FEE).build(),
    )
    .build();

    // Consume votes (transfer vote_in to vote_out)
    let consume_votes_tx = &mut consume_votes_tx(
        &algod,
        params.clone(),
        votes_in_escrow.address,
        votes_asset_id,
        votes_out_escrow.address,
    )
    .await?;

    // The creator pays the fee of the votes tx (signed by vote_in)
    let pay_vote_fee_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(creator, votes_in_escrow.address, FIXED_FEE).build(),
    )
    .build();

    TxGroup::assign_group_id(vec![
        withdraw_tx,
        pay_withdraw_fee_tx,
        consume_votes_tx,
        pay_vote_fee_tx,
    ])?;

    let signed_withdraw_tx = central_escrow.sign(withdraw_tx, vec![])?;
    let signed_consume_votes_tx = votes_in_escrow.sign(consume_votes_tx, vec![])?;

    Ok(WithdrawToSign {
        withdraw_tx: signed_withdraw_tx,
        pay_withdraw_fee_tx: pay_withdraw_fee_tx.clone(),
        consume_votes_tx: signed_consume_votes_tx,
        pay_vote_fee_tx: pay_vote_fee_tx.clone(),
    })
}

/// Transfers all vote tokens from votes_in to votes_out
/// The votes_in escrow logic controls whether this transactions passes or not:
/// The transfer will be approved if tokens count is > threshold (i.e. "enough votes for withdrawal")
async fn consume_votes_tx(
    algod: &Algod,
    params: SuggestedTransactionParams,
    votes_in_escrow: Address,
    votes_asset_id: u64,
    votes_out_escrow: Address,
) -> Result<Transaction> {
    // Get the vote tokens count in the votes in escrow (to attempt to transfer everything)
    let votes_in_account = algod.account_information(&votes_in_escrow).await?;
    let investor_votes = votes_in_account
        .assets
        .iter()
        .find(|a| a.asset_id == votes_asset_id)
        // TODO confirm that this means not opted in,
        .ok_or(anyhow!("Votes_in doesn't have vote asset"))?;
    let votes_count = investor_votes.amount;

    let tx = TxnBuilder::with(
        params,
        TransferAsset::new(
            votes_in_escrow,
            votes_asset_id,
            votes_count,
            votes_out_escrow,
        )
        .build(),
    )
    .build();

    Ok(tx)
}

pub async fn submit_withdraw(algod: &Algod, signed: &WithdrawSigned) -> Result<String> {
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.withdraw_tx.clone(),
    //         signed.pay_withdraw_fee_tx.clone(),
    //         signed.consume_votes_tx.clone(),
    //         signed.pay_vote_fee_tx.clone(),
    //     ],
    //     "voting_in_escrow",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[
            signed.withdraw_tx.clone(),
            signed.pay_withdraw_fee_tx.clone(),
            signed.consume_votes_tx.clone(),
            signed.pay_vote_fee_tx.clone(),
        ])
        .await?;
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawToSign {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: Transaction,
    pub consume_votes_tx: SignedTransaction,
    pub pay_vote_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawSigned {
    pub withdraw_tx: SignedTransaction,
    pub pay_withdraw_fee_tx: SignedTransaction,
    pub consume_votes_tx: SignedTransaction,
    pub pay_vote_fee_tx: SignedTransaction,
}

#[cfg(test)]
mod tests {
    use algonaut::core::MicroAlgos;
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::withdraw::logic::{AssetHolder, FIXED_FEE},
        testing::{
            flow::withdraw::{withdraw_flow, withdraw_precs},
            network_test_util::reset_network,
            test_data::{creator, customer, investor1, investor2, project_specs},
        },
    };

    #[test]
    #[serial]
    async fn test_withdraw() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let drainer = investor1();
        let voter = investor2();
        let customer = customer();

        // flow

        let pay_and_drain_amount = MicroAlgos(10 * 1_000_000);
        let precs = withdraw_precs(
            &algod,
            &creator,
            &project_specs(),
            &drainer,
            &customer,
            &voter,
            pay_and_drain_amount,
        )
        .await?;

        // remeber state
        let vote_in_after_voting = algod
            .account_information(&precs.project.votein_escrow.address)
            .await?;
        let vote_in_after_voting_holding =
            vote_in_after_voting.get_holdings(precs.project.votes_asset_id);
        assert!(vote_in_after_voting_holding.is_some()); // double check
        let vote_in_after_voting_amount = vote_in_after_voting_holding.unwrap().amount;

        // remeber state
        let initial_central_balance = algod
            .account_information(&precs.project.central_escrow.address)
            .await?
            .amount;

        // remeber state
        let creator_balance_bafore_withdrawing =
            algod.account_information(&creator.address()).await?.amount;

        let withdraw_amount = MicroAlgos(1_000_000); // UI
        let _res = withdraw_flow(&algod, &precs.project, &creator, withdraw_amount).await?;

        // test

        // creator got the amount and lost the fees for the withdraw tx (central -> creator) and vote tx (vote_in -> vote_out) and the fees for these 2 fees-payment txs
        let withdrawer_account = algod.account_information(&creator.address()).await?;
        assert_eq!(
            creator_balance_bafore_withdrawing + withdraw_amount - FIXED_FEE * 4,
            withdrawer_account.amount
        );

        // central lost the withdrawn amount
        let central_escrow_balance = algod
            .account_information(&precs.project.central_escrow.address)
            .await?
            .amount;
        assert_eq!(
            initial_central_balance - withdraw_amount,
            central_escrow_balance
        );

        // votes transferred from vote_in to vote_out
        // vote_in has no votes
        let vote_in_escrow = algod
            .account_information(&precs.project.votein_escrow.address)
            .await?;
        let vote_in_asset_holding = vote_in_escrow.get_holdings(precs.project.votes_asset_id);
        assert!(vote_in_asset_holding.is_some());
        assert_eq!(0, vote_in_asset_holding.unwrap().amount);
        // vote_out has all the votes
        let vote_out_escrow = algod
            .account_information(&precs.project.vote_out_escrow.address)
            .await?;
        let vote_out_asset_holding = vote_out_escrow.get_holdings(precs.project.votes_asset_id);
        assert!(vote_out_asset_holding.is_some());
        assert_eq!(
            vote_in_after_voting_amount,
            vote_out_asset_holding.unwrap().amount
        );

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
