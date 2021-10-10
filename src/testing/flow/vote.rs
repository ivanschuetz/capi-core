#[cfg(test)]
use super::invest_in_project::invests_flow;
#[cfg(test)]
use crate::flows::{
    create_project::model::Project,
    vote::logic::{submit_vote, vote, VoteSigned},
};
#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn vote_flow(
    algod: &Algod,
    voter: &Account,
    project: &Project,
    buy_asset_amount: u64,
) -> Result<VoteTestFlowRes> {
    // Investor buys shares with count == vote threshold count
    let _ = invests_flow(algod, voter, buy_asset_amount, &project).await?;
    // Sends (all) the votes to make the vote successful
    let vote_to_sign = vote(
        algod,
        voter.address(),
        project.votes_asset_id,
        project.central_app_id,
        &project.staking_escrow,
        buy_asset_amount,
        project.votein_escrow.address,
    )
    .await?;

    // UI

    let signed_validate_investor_vote_count_tx =
        voter.sign_transaction(&vote_to_sign.validate_investor_vote_count_tx)?;

    let vote_tx_id = submit_vote(
        &algod,
        &VoteSigned {
            validate_investor_vote_count_tx: signed_validate_investor_vote_count_tx,
            xfer_tx: vote_to_sign.xfer_tx,
        },
    )
    .await?;

    wait_for_pending_transaction(&algod, &vote_tx_id).await?;

    Ok(VoteTestFlowRes {})
}

#[cfg(test)]
// Any data we want to return from the flow to the tests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteTestFlowRes {}
