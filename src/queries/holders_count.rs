use algonaut::{core::MicroAlgos, indexer::v2::Indexer, model::indexer::v2::QueryAccount};
use anyhow::Result;

pub async fn holders_count(indexer: &Indexer, asset_id: u64) -> Result<usize> {
    let accounts = indexer
        .accounts(&QueryAccount {
            asset_id: Some(asset_id),
            ..QueryAccount::default()
        })
        .await?;

    log::debug!("Counting holders: {:?}", accounts);
    Ok(accounts
        .accounts
        .iter()
        // if accounts have no assets but are opted in, we get 0 count - filter those out
        .filter(|a| a.amount > MicroAlgos(0))
        .count())
}
