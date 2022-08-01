use crate::flows::{
    create_dao::storage::load_dao::load_dao,
    withdraw::note::base64_withdrawal_note_to_withdrawal_description,
};
use algonaut::{
    algod::v2::Algod, core::Address, indexer::v2::Indexer,
    model::indexer::v2::QueryAccountTransaction,
};
use anyhow::{anyhow, Error, Result};
use chrono::{DateTime, Utc};
use mbase::date_util::timestamp_seconds_to_date;
use mbase::models::tx_id::TxId;
use mbase::models::{
    dao_id::DaoId,
    funds::{FundsAmount, FundsAssetId},
};
use serde::{Deserialize, Serialize};

#[allow(clippy::too_many_arguments)]
pub async fn withdrawals(
    algod: &Algod,
    indexer: &Indexer,
    dao_id: DaoId,
    funds_asset: FundsAssetId,
    before_time: &Option<DateTime<Utc>>,
    after_time: &Option<DateTime<Utc>>,
) -> Result<Vec<Withdrawal>> {
    let dao = load_dao(algod, dao_id).await?;

    // let before_time_formatted = before_time.map(|t| t.to_rfc3339());
    // let after_time_formatted = after_time.map(|t| t.to_rfc3339());

    let query = QueryAccountTransaction {
        // TODO filter by application id here?
        // added to disabled_parameters..
        // tx_type: Some(TransactionType::ApplicationTransaction),
        // added to disabled_parameters..
        // before_time: before_time_formatted,
        // added to disabled_parameters..
        // after_time: after_time_formatted,
        // For now no prefix filtering
        // Algorand's indexer has performance problems with note-prefix and it doesn't work at all with AlgoExplorer or PureStake currently:
        // https://github.com/algorand/indexer/issues/358
        // https://github.com/algorand/indexer/issues/669
        // note_prefix: Some(withdraw_note_prefix_base64()),
        ..Default::default()
    };

    // TODO filter txs by receiver (creator) - this returns everything associated with creator
    let txs = indexer
        .account_transactions(&dao.owner, &query)
        .await?
        .transactions;

    // TODO (low prio) compare performance of above vs this (i.e. querying account txs vs txs with receiver field)
    // Note that none is using note prefix currently, see note in query above.
    // let query = QueryTransaction {
    //     address: Some(creator.to_string()),
    //     address_role: Some(Role::Receiver),
    //     ..Default::default()
    // };
    // let txs = indexer.transactions(&query).await?.transactions;

    let mut withdrawals = vec![];

    for tx in &txs {
        if let Some(app_call) = tx.application_transaction.clone() {
            if app_call.application_id == dao.app_id.0 {
                for inner_tx in &tx.inner_txns {
                    // withdrawals are xfers from the app to the owner
                    if let Some(xfer) = inner_tx.asset_transfer_transaction.clone() {
                        // not sure that we need to check the sender here - it's probably always the app? but it doesn't hurt
                        let sender_address =
                            inner_tx.sender.parse::<Address>().map_err(Error::msg)?;
                        let receiver_address =
                            xfer.receiver.parse::<Address>().map_err(Error::msg)?;

                        // account_transactions returns all the txs "related" to the account, i.e. can be sender or receiver
                        // we're interested only in central escrow -> creator
                        if FundsAssetId(xfer.asset_id) == funds_asset
                            && sender_address == dao.app_address()
                            && receiver_address == dao.owner
                        {
                            // for now the only payload is the description
                            let withdrawal_description = match &tx.note {
                                Some(note) => {
                                    base64_withdrawal_note_to_withdrawal_description(note)?
                                }
                                None => "".to_owned(),
                            };

                            // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
                            // Unclear when it's None. For now we just reject it.
                            let round_time = tx.round_time.ok_or_else(|| {
                                anyhow!("Unexpected: tx has no round time: {:?}", tx)
                            })?;

                            let id = tx
                                .id
                                .clone()
                                .ok_or_else(|| anyhow!("Unexpected: tx has no id: {:?}", tx))?;

                            // needs to be checked manually, because the query param was disabled
                            if let Some(after_time) = after_time {
                                if round_time < after_time.timestamp() as u64 {
                                    continue;
                                }
                            }
                            // needs to be checked manually, because the query param was disabled
                            if let Some(before_time) = before_time {
                                if round_time > before_time.timestamp() as u64 {
                                    continue;
                                }
                            }

                            withdrawals.push(Withdrawal {
                                amount: FundsAmount::new(xfer.amount),
                                description: withdrawal_description,
                                date: timestamp_seconds_to_date(round_time)?,
                                tx_id: id.parse()?,
                                // this should be always the owner - we return it for the UI, which currently shows addresses for all the activity entries
                                address: receiver_address,
                            })
                        }
                    }
                }
            }
        }
    }

    Ok(withdrawals)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Withdrawal {
    pub amount: FundsAmount,
    pub description: String,
    pub date: DateTime<Utc>,
    pub tx_id: TxId,
    pub address: Address,
}
