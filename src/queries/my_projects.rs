use std::collections::HashMap;

use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos},
    indexer::v2::Indexer,
    model::indexer::v2::{QueryTransaction, Role},
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};

use crate::{
    date_util::timestamp_seconds_to_date,
    flows::create_project::{
        create_project::Escrows,
        model::Project,
        storage::{
            load_project::{load_project, ProjectId},
            note::base64_note_to_project,
        },
    },
    state::central_app_state::find_state_with_a_capi_project_id,
};

// TODO use StoredProject where applicable
// TODO move this somewhere else
#[derive(Debug, Clone)]
pub struct StoredProject {
    pub id: ProjectId,
    pub project: Project,
    pub stored_date: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MyStoredProject {
    pub project: StoredProject,
    // whether I created this project
    pub created_by_me: bool,
    // whether I'm currently invested (staking shares) in this project
    pub invested_by_me: bool,
}

pub async fn my_projects(
    algod: &Algod,
    indexer: &Indexer,
    address: &Address,
    escrows: &Escrows,
) -> Result<Vec<MyStoredProject>> {
    let created = my_created_projects(algod, indexer, address, escrows).await?;
    let invested = my_current_invested_projects(algod, indexer, address, escrows).await?;

    let created_map: HashMap<ProjectId, StoredProject> = created
        .iter()
        .map(|a| (a.id.clone(), a.to_owned()))
        .collect();

    let invested_map: HashMap<ProjectId, StoredProject> = invested
        .iter()
        .map(|a| (a.id.clone(), a.to_owned()))
        .collect();

    // Projects created by me and [created and invested] by me
    let mut projects = vec![];
    for project in created {
        let invested_by_me = invested_map.contains_key(&project.id);
        projects.push(MyStoredProject {
            project,
            created_by_me: true,
            invested_by_me,
        });
    }

    // Projects only invested by me
    for project in invested {
        if !created_map.contains_key(&project.id) {
            projects.push(MyStoredProject {
                project,
                created_by_me: false,
                invested_by_me: true,
            });
        }
    }

    // sort ascendingly by date
    projects.sort_by(|p1, p2| p1.project.stored_date.cmp(&p2.project.stored_date));

    Ok(projects)
}

/// Returns projects where the user is invested. Meaning: has currently staked shares (more exactly a local state containing the project id).
/// (Projects for non-staked shares, where the user opted out, or where the local state was deleted (externally) don't count).
pub async fn my_current_invested_projects(
    algod: &Algod,
    indexer: &Indexer,
    address: &Address,
    escrows: &Escrows,
) -> Result<Vec<StoredProject>> {
    log::debug!("Retrieving my project from: {:?}", address);

    let account = algod.account_information(address).await?;

    let apps = account.apps_local_state;

    // Check the local state from all the apps that the user is opted in for capi project ids and collect them
    let mut my_project_ids = vec![];
    for app in apps {
        if let Some(project_id) = find_state_with_a_capi_project_id(&app)? {
            my_project_ids.push(project_id)
        }
    }

    let mut my_projects = vec![];
    for project_id in my_project_ids {
        // If there's a project id and there are no bugs, there should *always* be a project - as the ids are on-chain tx ids
        // and these tx should have the properly formatted project data in the note field
        let project = load_project(algod, indexer, &project_id, escrows).await?;
        my_projects.push(project);
    }

    Ok(my_projects)
}

/// Returns projects created by user (this is technically defined as projects where user was the sender of the store project tx)
/// Note that this currently doesn't consider the case that the project might be considered as "deleted"
/// (this use case hasn't even been considered at all, also not in the UI/UX)
// TODO (low prio): review: if for some weird reason one user creates the asset and initializes the contracts and another stores the project,
// what are the consequences? any possible security or UX issues?
// Consider all combinations, e.g. 3 differnet users to these actions respectively
pub async fn my_created_projects(
    algod: &Algod,
    indexer: &Indexer,
    address: &Address,
    escrows: &Escrows,
) -> Result<Vec<StoredProject>> {
    log::debug!("Retrieving my project from: {:?}", address);

    let response = indexer
        .transactions(&QueryTransaction {
            address: Some(address.to_string()),
            address_role: Some(Role::Sender),
            // TODO later we can use a note prefix to make this more performant. Currently Algorand's indexer has performance issues with the indexer query and it doesn't with on third parties.
            ..QueryTransaction::default()
        })
        .await?;

    let mut my_projects = vec![];

    for tx in response.transactions {
        if tx.payment_transaction.is_some() {
            if let Some(note) = &tx.note {
                if !note.is_empty() {
                    match base64_note_to_project(algod, escrows, note).await {
                        Ok(project) => {
                            // Round time is documented as optional (https://developer.algorand.org/docs/rest-apis/indexer/#transaction)
                            // Unclear when it's None. For now we just reject it.
                            let round_time = tx.round_time.ok_or_else(|| {
                                anyhow!("Unexpected: tx has no round time: {:?}", tx)
                            })?;

                            my_projects.push(StoredProject {
                                id: tx.id.parse()?,
                                project,
                                stored_date: timestamp_seconds_to_date(round_time)?,
                            });
                        }
                        Err(_e) => {
                            // for now we'll assume that if the note can't be parsed, the tx is not a project storage tx
                            // TODO that's of course incorrect - it can't be parsed e.g. because the payload is incorrectly formatted
                            // and we should return an error in these cases.
                            // ideally these notes should have a prefix to identify as capi/project storage
                            // so if it doesn't have the prefix, we can safely ignore, otherwise treat errors as actual errors.
                            log::trace!("Checking user's txs for project creation txs: User sent a non-project storage cration tx.")
                        }
                    }
                }
            }
        }
    }

    Ok(my_projects)
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub amount: MicroAlgos,
    pub sender: Address,
    pub date: DateTime<Utc>,
    pub note: Option<String>,
}
