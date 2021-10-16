#[cfg(test)]
use crate::flows::{
    create_project::model::Project,
    vote::logic::{submit_vote, vote, VoteSigned},
};
#[cfg(test)]
use algonaut::{algod::v2::Algod, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn vote_flow(
    algod: &Algod,
    voter: &Account,
    project: &Project,
    slot_id: u64,
    vote_count: u64,
) -> Result<String> {
    let vote_to_sign = vote(
        &algod,
        voter.address(),
        project.central_app_id,
        slot_id,
        vote_count,
    )
    .await?;

    let signed_vote_tx = voter.sign_transaction(&vote_to_sign.vote_tx)?;
    let signed_validate_vote_count_tx =
        voter.sign_transaction(&vote_to_sign.validate_vote_count_tx)?;
    let tx_id = submit_vote(
        &algod,
        &VoteSigned {
            vote_tx: signed_vote_tx,
            validate_vote_count_tx: signed_validate_vote_count_tx,
        },
    )
    .await?;

    Ok(tx_id)
}
