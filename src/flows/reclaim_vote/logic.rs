use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
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

/// Transfers all the votes from vote_out to staking escrow
pub async fn reclaim_votes(
    algod: &Algod,
    reclaimer: Address,
    votes_asset_id: u64,
    vote_out_escrow: &ContractAccount,
    staking_escrow: &ContractAccount,
) -> Result<ReclaimVotesToSign> {
    let params = algod.suggested_transaction_params().await?;

    // Get vote count in vote_out escrow
    let vote_out_account = algod.account_information(&vote_out_escrow.address).await?;
    let votes_to_reclaim_count = vote_out_account
        .assets
        .iter()
        .find(|a| a.asset_id == votes_asset_id)
        // TODO confirm that this means not opted in,
        .ok_or_else(|| anyhow!(
            "vote_out doesn't have votes (TODO confirm that this means not opted in, not 0, edit msg)"
        ))?
        .amount;

    // The votes that can be reclaimed is the shares count
    // The vote_out escrow also checks that the user doesn't have vote tokens (otherwise could simply claim again to get votes > shares)

    // i.e. when voting, all votes (== share count) are transferred to vote_in (checked by vote_in escrow)
    // when claiming, also all votes (== share count) have to be claimed (checked by vote_out escrow)
    // TODO (after staking) review if there can be situations where investor is left with incomplete (non zero) votes.
    println!(
        "Reclaim: Creating tx to transfer {:?} votes from vote_out to staking escrow",
        votes_to_reclaim_count
    );

    // Transfer all vote tokens to the staking escrow
    let votes_xfer_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        TransferAsset::new(
            vote_out_escrow.address,
            votes_asset_id,
            votes_to_reclaim_count,
            staking_escrow.address,
        )
        .build(),
    )
    .build();

    // Pay for the vote tokens transfer tx
    let pay_votex_xfer_fee_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        Pay::new(reclaimer, vote_out_escrow.address, FIXED_FEE).build(),
    )
    .build();

    TxGroup::assign_group_id(vec![votes_xfer_tx, pay_votex_xfer_fee_tx])?;

    let signed_votes_xfer_tx = vote_out_escrow.sign(votes_xfer_tx, vec![])?;

    Ok(ReclaimVotesToSign {
        votes_xfer_tx: signed_votes_xfer_tx,
        pay_votes_xfer_fee_tx: pay_votex_xfer_fee_tx.clone(),
    })
}

pub async fn submit_reclaim_votes(algod: &Algod, signed: ReclaimVotesSigned) -> Result<String> {
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.votes_xfer_tx_signed.clone(),
    //         signed.pay_votes_xfer_fee_tx.clone(),
    //     ],
    //     "voting_out_escrow",
    // )
    // .unwrap();
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.votes_xfer_tx_signed.clone(),
    //         signed.pay_votes_xfer_fee_tx.clone(),
    //     ],
    //     "voting_out_app",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[signed.votes_xfer_tx_signed, signed.pay_votes_xfer_fee_tx])
        .await?;
    println!("Reclaim votes tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimVotesToSign {
    /// Votes transfer (vote_out -> investor)
    pub votes_xfer_tx: SignedTransaction,
    /// Pay votes transfer fee
    pub pay_votes_xfer_fee_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimVotesSigned {
    pub votes_xfer_tx_signed: SignedTransaction,
    pub pay_votes_xfer_fee_tx: SignedTransaction,
}

#[cfg(test)]
mod tests {
    use algonaut::{
        core::MicroAlgos,
        transaction::{Pay, TxnBuilder},
    };
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::reclaim_vote::logic::{
            reclaim_votes, submit_reclaim_votes, ReclaimVotesSigned, FIXED_FEE,
        },
        network_util::wait_for_pending_transaction,
        testing::{
            flow::{create_project::create_project_flow, vote::vote_flow, withdraw::withdraw_flow},
            network_test_util::reset_network,
            test_data::{creator, investor1, project_specs},
        },
    };

    #[test]
    #[serial]
    async fn test_reclaim_vote() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let investor = investor1();

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs(), 3).await?;
        // enough units for vote to pass
        let buy_asset_amount = project.specs.vote_threshold_units();
        let _ = vote_flow(&algod, &investor, &project, buy_asset_amount).await?;

        let vote_in_after_voting = algod
            .account_information(&project.votein_escrow.address)
            .await?;
        println!("vote_in_after_voting: {:?}", vote_in_after_voting);

        let amount_to_withdraw = MicroAlgos(1_000);

        // fund the central (directly: not using drain to keep the test focused)
        let params = algod.suggested_transaction_params().await?;
        let fund_central_algos_tx = TxnBuilder::with(
            params.clone(),
            Pay::new(
                creator.address(),
                project.central_escrow.address,
                amount_to_withdraw,
            )
            .build(),
        )
        .build();
        let signed_fund_central_algos_tx = creator.sign_transaction(&fund_central_algos_tx)?;
        let signed_fund_central_algos_res = algod
            .broadcast_signed_transaction(&signed_fund_central_algos_tx)
            .await?;
        let _ = wait_for_pending_transaction(&algod, &signed_fund_central_algos_res.tx_id).await?;

        // withdrawing moves all the votes from vote_in to vote_out
        let _ = withdraw_flow(&algod, &project, &creator, amount_to_withdraw).await?;

        // flow

        // save
        let investor_infos_before_reclaim = algod.account_information(&investor.address()).await?;
        let vote_out_infos_before_reclaim = algod
            .account_information(&project.vote_out_escrow.address)
            .await?;

        // note that anyone can reclaim (even users that have nothing to do with the app)
        // this is just a "service" to make the votes available again,
        // usually an investor who wants to vote will trigger it
        let to_sign = reclaim_votes(
            &algod,
            investor.address(),
            project.votes_asset_id,
            &project.vote_out_escrow,
            &project.staking_escrow,
        )
        .await?;

        let signed_reclaim_pay_votes_xfer_fee_tx =
            investor.sign_transaction(&to_sign.pay_votes_xfer_fee_tx)?;

        let tx_id = submit_reclaim_votes(
            &algod,
            ReclaimVotesSigned {
                votes_xfer_tx_signed: to_sign.votes_xfer_tx,
                pay_votes_xfer_fee_tx: signed_reclaim_pay_votes_xfer_fee_tx,
            },
        )
        .await?;

        wait_for_pending_transaction(&algod, &tx_id).await?;

        // test

        // double check: there are no votes in votes_in
        let vote_in_infos = algod
            .account_information(&project.votein_escrow.address)
            .await?;
        assert_eq!(1, vote_in_infos.assets.len());
        assert_eq!(0, vote_in_infos.assets[0].amount);

        // votes_out lost the votes
        let vote_out_infos = algod
            .account_information(&project.vote_out_escrow.address)
            .await?;
        assert_eq!(1, vote_out_infos.assets.len());
        assert_eq!(0, vote_out_infos.assets[0].amount);

        // staking escrow got the votes
        let escrow_infos = algod
            .account_information(&project.staking_escrow.address)
            .await?;
        assert_eq!(2, escrow_infos.assets.len()); // still opted in to shares and votes
        assert_eq!(buy_asset_amount, escrow_infos.assets[0].amount); // the shares are still staked
        assert_eq!(buy_asset_amount, escrow_infos.assets[1].amount); // the reclaimed votes were transferred

        // double check: investor still doesn't have any tokens
        let investor_infos = algod.account_information(&investor.address()).await?;
        assert_eq!(1, investor_infos.assets.len());
        assert_eq!(0, investor_infos.assets[0].amount);

        // investor paid for votes transfer fee + fee for votes transfer fee tx
        assert_eq!(
            investor_infos_before_reclaim.amount - FIXED_FEE * 2,
            investor_infos.amount
        );

        // escrow algos balance unchanged (since investor paid for the fee)
        let vote_out_infos = algod
            .account_information(&project.vote_out_escrow.address)
            .await?;
        assert_eq!(vote_out_infos_before_reclaim.amount, vote_out_infos.amount);

        Ok(())
    }
}
