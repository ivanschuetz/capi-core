use crate::{
    capi_deps::CapiAssetDaoDeps,
    flows::create_dao::{model::Dao, storage::load_dao::load_dao},
    note::dao_setup_prefix_base64,
    teal::TealApi,
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    indexer::v2::Indexer,
    model::indexer::v2::{OnCompletion, QueryTransaction},
};
use anyhow::{anyhow, Error, Result};
use chrono::{DateTime, Utc};
use mbase::{
    models::{dao_app_id::DaoAppId, dao_id::DaoId},
    state::dao_app_state::matches_capi_local_state,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MyStoredDao {
    pub dao: Dao,
    // whether I created this dao
    pub created_by_me: bool,
    // whether I'm currently invested (locking shares) in this dao
    pub invested_by_me: bool,
}

pub async fn my_daos(
    algod: &Algod,
    indexer: &Indexer,
    address: &Address,
    api: &dyn TealApi,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Vec<MyStoredDao>> {
    let created = my_created_daos(algod, indexer, address, api, capi_deps).await?;
    let invested = my_current_invested_daos(algod, address, api, capi_deps).await?;

    let created_map: HashMap<DaoId, Dao> = created.iter().map(|d| (d.id(), d.to_owned())).collect();

    let invested_map: HashMap<DaoId, Dao> =
        invested.iter().map(|d| (d.id(), d.to_owned())).collect();

    // Daos created by me and [created and invested] by me
    let mut daos = vec![];
    for dao in created {
        let invested_by_me = invested_map.contains_key(&dao.id());
        daos.push(MyStoredDao {
            dao,
            created_by_me: true,
            invested_by_me,
        });
    }

    // Daos only invested by me
    for dao in invested {
        if !created_map.contains_key(&dao.id()) {
            daos.push(MyStoredDao {
                dao,
                created_by_me: false,
                invested_by_me: true,
            });
        }
    }

    Ok(daos)
}

/// Returns daos where the user is invested. Meaning: has currently locked shares (more exactly a local state containing the dao id).
/// (Daos for non-locked shares, where the user opted out, or where the local state was deleted (externally) don't count).
/// TODO can this be improved, now that we use URL->app id?
pub async fn my_current_invested_daos(
    algod: &Algod,
    address: &Address,
    api: &dyn TealApi,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Vec<Dao>> {
    log::debug!("Retrieving my current invested daos from: {:?}", address);

    let account = algod.account_information(address).await?;

    let apps = account.apps_local_state;

    // Check the local state from all the apps that the user is opted in for capi dao ids and collect them
    let mut my_dao_ids = vec![];
    for app in apps {
        if matches_capi_local_state(&app) {
            my_dao_ids.push(DaoId(DaoAppId(app.id)))
        }
    }

    let mut my_daos = vec![];
    for dao_id in my_dao_ids {
        // If there's a dao id and there are no bugs, there should *always* be a dao - as the ids are on-chain tx ids
        // and these tx should have the properly formatted dao data in the note field
        let dao = load_dao(algod, dao_id, api, capi_deps).await?;
        my_daos.push(dao);
    }

    Ok(my_daos)
}

/// Returns daos created by user (this is technically defined as daos where user was the sender of the store dao tx)
/// Note that this currently doesn't consider the case that the dao might be considered as "deleted"
/// (this use case hasn't even been considered at all, also not in the UI/UX)
// TODO (low prio): review: if for some weird reason one user creates the asset and initializes the contracts and another stores the dao,
// what are the consequences? any possible security or UX issues?
// Consider all combinations, e.g. 3 differnet users to these actions respectively
pub async fn my_created_daos(
    algod: &Algod,
    indexer: &Indexer,
    address: &Address,
    api: &dyn TealApi,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Vec<Dao>> {
    log::debug!("Retrieving my created daos from: {:?}", address);

    let response = indexer
        .transactions(&QueryTransaction {
            address: Some(address.to_string()),
            // indexer disabled this, for performance apparently https://github.com/algorand/indexer/commit/1216e7957d5fba7c6a858e244a2aaf7e99412e5d
            // so we filter locally
            // address_role: Some(Role::Sender),
            // TODO later we can use a note prefix to make this more performant. Currently Algorand's indexer has performance issues with the indexer query and it doesn't with on third parties.
            ..QueryTransaction::default()
        })
        .await?;

    let mut my_daos = vec![];

    for tx in response.transactions {
        let sender_address = tx.sender.parse::<Address>().map_err(Error::msg)?;
        if &sender_address == address {
            if let Some(app_tx) = &tx.application_transaction {
                if app_tx.on_completion == OnCompletion::Noop {
                    // Filter out app calls that are Capi DAO setups
                    // these transactions are unique per sender-dao and give us the app id (dao id)
                    if tx.note == Some(dao_setup_prefix_base64()) {
                        let app_id = app_tx.application_id;
                        if app_id == 0 {
                            return Err(anyhow!(
                                "Invalid state: Found 0 app id fetching dao setup transactions. Tx: {tx:?}"
                            ));
                        }
                        let dao_id = DaoId(DaoAppId(app_id));

                        let dao = load_dao(algod, dao_id, api, capi_deps).await?;
                        my_daos.push(dao);
                    }
                }
            }
        }
    }

    Ok(my_daos)
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub amount: MicroAlgos,
    pub sender: Address,
    pub date: DateTime<Utc>,
    pub note: Option<String>,
}
