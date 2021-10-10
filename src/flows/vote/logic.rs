use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        account::ContractAccount, builder::CallApplication, tx_group::TxGroup, SignedTransaction,
        Transaction, TransferAsset, TxnBuilder,
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
    investor: Address,
    votes_asset_id: u64,
    central_app_id: u64,
    staking_escrow: &ContractAccount,
    votes_count: u64, // teal expects this to be == shares in local state
    votesin_escrow: Address,
) -> Result<VoteToSign> {
    let params = algod.suggested_transaction_params().await?;

    // App call to check that the votes being transferred are == to owned shares (local state)
    let validate_investor_vote_count_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        CallApplication::new(investor, central_app_id)
            .app_arguments(vec!["validate_investor_votes".as_bytes().to_vec()])
            .build(),
    )
    .build();

    // TODO tx to pay for this tx fee
    // Transfer all the vote tokens to the votes_in escrow
    let votes_xfer_tx = &mut TxnBuilder::with(
        SuggestedTransactionParams {
            fee: FIXED_FEE,
            ..params.clone()
        },
        TransferAsset::new(
            staking_escrow.address,
            votes_asset_id,
            votes_count,
            votesin_escrow,
        )
        .build(),
    )
    .build();

    TxGroup::assign_group_id(vec![validate_investor_vote_count_tx, votes_xfer_tx])?;

    let signed_xfer_tx = staking_escrow.sign(votes_xfer_tx, vec![])?;

    Ok(VoteToSign {
        validate_investor_vote_count_tx: validate_investor_vote_count_tx.clone(),
        xfer_tx: signed_xfer_tx,
    })
}

pub async fn submit_vote(algod: &Algod, signed: &VoteSigned) -> Result<String> {
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.validate_investor_vote_count_tx.clone(),
    //         signed.xfer_tx.clone(),
    //     ],
    //     "app_central_approval",
    // )
    // .unwrap();
    // crate::teal::debug_teal_rendered(
    //     &[
    //         signed.validate_investor_vote_count_tx.clone(),
    //         signed.xfer_tx.clone(),
    //     ],
    //     "staking_escrow",
    // )
    // .unwrap();

    let res = algod
        .broadcast_signed_transactions(&[
            signed.validate_investor_vote_count_tx.clone(),
            signed.xfer_tx.clone(),
        ])
        .await?;
    println!("Vote tx id: {:?}", res.tx_id);
    Ok(res.tx_id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteToSign {
    pub validate_investor_vote_count_tx: Transaction,
    pub xfer_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteSigned {
    pub validate_investor_vote_count_tx: SignedTransaction,
    pub xfer_tx: SignedTransaction,
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serial_test::serial;
    use tokio::test;

    use crate::{
        dependencies,
        flows::vote::logic::{submit_vote, VoteSigned},
        network_util::wait_for_pending_transaction,
        testing::{
            flow::{create_project::create_project_flow, invest_in_project::invests_flow},
            network_test_util::reset_network,
            test_data::{creator, investor1, project_specs},
        },
    };

    use super::vote;

    #[test]
    #[serial]
    async fn test_vote() -> Result<()> {
        reset_network()?;

        // deps

        let algod = dependencies::algod();
        // anyone can drain (they've to pay the fee): it will often be an investor, to be able to harvest
        let creator = creator();
        let investor = investor1();

        // precs

        let project = create_project_flow(&algod, &creator, &project_specs()).await?;
        let buy_asset_amount = 10;
        let _ = invests_flow(&algod, &investor, buy_asset_amount, &project).await?;

        // flow

        // in the real application, votes_count is retrieved from indexer
        // users sends all their votes when voting
        // the smart contracts validate this (shares count local state == votes xfer) // TODO verify this
        // (partial voting doesn't make sense: vote tokens represent essentially the weight of the voter, not "independent units",
        // users want either to approve (send all votes) or not approve (send no votes) a withdrawal).
        // TODO verify that all votes are sent and ensure at least in the UI no UX problems with missing vote tokens
        // (because they're not in the staking contract but vote_in or vote_out)
        let votes_count = buy_asset_amount;

        let vote_to_sign = vote(
            &algod,
            investor.address(),
            project.votes_asset_id,
            project.central_app_id,
            &project.staking_escrow,
            votes_count,
            project.votein_escrow.address,
        )
        .await?;

        let signed_validate_investor_vote_count_tx =
            investor.sign_transaction(&vote_to_sign.validate_investor_vote_count_tx)?;
        let tx_id = submit_vote(
            &algod,
            &VoteSigned {
                validate_investor_vote_count_tx: signed_validate_investor_vote_count_tx,
                xfer_tx: vote_to_sign.xfer_tx,
            },
        )
        .await?;

        wait_for_pending_transaction(&algod, &tx_id).await?;

        // test

        let votein_infos = algod
            .account_information(&project.votein_escrow.address)
            .await?;

        assert_eq!(1, votein_infos.assets.len());
        // investor bought buy_asset_amount shares, so they got buy_asset_amount votes
        // and voting sends all the votes to the votein escrow, so the votein escrow should have not buy_asset_amount votes
        assert_eq!(buy_asset_amount, votein_infos.assets[0].amount);
        assert_eq!(project.votes_asset_id, votein_infos.assets[0].asset_id);
        // Just checking the expected value for other fields, no special reason
        assert_eq!(creator.address(), votein_infos.assets[0].creator); // the project creator created the voting assets
        assert_eq!(false, votein_infos.assets[0].is_frozen);

        Ok(())
    }
}
