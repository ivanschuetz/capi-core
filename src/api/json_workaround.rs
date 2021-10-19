use std::convert::{TryFrom, TryInto};

use super::model::{DefaultError, ProjectForUsers};
use crate::flows::create_project::model::{CreateProjectSpecs, Project};
use algonaut::{
    core::{CompiledTealBytes, MicroAlgos},
    transaction::account::ContractAccount,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/////////////////////////////////////////////////////////////////////////////////////////////////
// workaround for some algonaut types not being serializable with json (only msg pack)
// we could serialize them with msg pack but for now json is better for debugging
// (e.g. web proxy, or in js for the wasm interface)
/////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractAccountJson {
    pub address: String,
    pub program: CompiledTealBytes,
}

impl From<ContractAccount> for ContractAccountJson {
    fn from(ca: ContractAccount) -> Self {
        ContractAccountJson {
            address: ca.address.to_string(),
            program: ca.program,
        }
    }
}

impl TryFrom<ContractAccountJson> for ContractAccount {
    type Error = DefaultError;

    fn try_from(ca: ContractAccountJson) -> Result<Self, Self::Error> {
        Ok(ContractAccount {
            address: ca.address.parse()?,
            program: ca.program,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectJson {
    pub specs: CreateProjectSpecs,
    pub shares_asset_id: u64,
    pub central_app_id: u64,
    pub withdrawal_slot_ids: Vec<u64>,
    pub invest_escrow: ContractAccountJson,
    pub staking_escrow: ContractAccountJson,
    pub central_escrow: ContractAccountJson,
    pub customer_escrow: ContractAccountJson,
    pub creator_address: String,
}

impl From<Project> for ProjectJson {
    fn from(p: Project) -> Self {
        ProjectJson {
            specs: p.specs,
            shares_asset_id: p.shares_asset_id,
            central_app_id: p.central_app_id,
            withdrawal_slot_ids: p.withdrawal_slot_ids,
            invest_escrow: p.invest_escrow.into(),
            staking_escrow: p.staking_escrow.into(),
            central_escrow: p.central_escrow.into(),
            customer_escrow: p.customer_escrow.into(),
            creator_address: p.creator.to_string(),
        }
    }
}

impl TryFrom<ProjectJson> for Project {
    type Error = DefaultError;

    fn try_from(p: ProjectJson) -> Result<Self, Self::Error> {
        Ok(Project {
            specs: p.specs,
            shares_asset_id: p.shares_asset_id,
            central_app_id: p.central_app_id,
            withdrawal_slot_ids: p.withdrawal_slot_ids,
            invest_escrow: p.invest_escrow.try_into()?,
            staking_escrow: p.staking_escrow.try_into()?,
            central_escrow: p.central_escrow.try_into()?,
            customer_escrow: p.customer_escrow.try_into()?,
            creator: p.creator_address.parse()?,
        })
    }
}

/// Note that we don't send things that can be queried from the blockchain,
/// like the asset name or supply
/// This is to minimize the off chain reponsibilities,
/// everything that can be queried directly from the blockchain should be (unless there's a very good reason not to)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectForUsersJson {
    pub id: String,
    pub name: String,
    pub asset_price: MicroAlgos,
    pub vote_threshold: String, // percent
    pub shares_asset_id: String,
    pub central_app_id: String,
    pub slot_ids: Vec<String>,
    pub invest_escrow_address: String,
    pub staking_escrow_address: String,
    pub central_escrow_address: String,
    pub customer_escrow_address: String,
    pub invest_link: String,
    pub my_investment_link: String,
    pub project_link: String,
    pub creator_address: String,
}

impl From<ProjectForUsers> for ProjectForUsersJson {
    fn from(p: ProjectForUsers) -> Self {
        ProjectForUsersJson {
            id: p.id.clone(),
            name: p.name.clone(),
            asset_price: p.asset_price,
            vote_threshold: p.vote_threshold.to_string(), // percent
            shares_asset_id: p.shares_asset_id.to_string(),
            central_app_id: p.central_app_id.to_string(),
            slot_ids: p.slot_ids.into_iter().map(|s| s.to_string()).collect(),
            invest_escrow_address: p.invest_escrow_address.to_string(),
            staking_escrow_address: p.staking_escrow_address.to_string(),
            central_escrow_address: p.central_escrow_address.to_string(),
            customer_escrow_address: p.customer_escrow_address.to_string(),
            invest_link: p.invest_link,
            my_investment_link: p.my_investment_link,
            project_link: p.project_link,
            creator_address: p.creator.to_string(),
        }
    }
}

impl TryFrom<ProjectForUsersJson> for ProjectForUsers {
    type Error = DefaultError;

    fn try_from(p: ProjectForUsersJson) -> Result<Self, Self::Error> {
        Ok(ProjectForUsers {
            id: p.id.clone(),
            name: p.name.clone(),
            asset_price: p.asset_price,
            vote_threshold: p.vote_threshold.parse()?, // percent
            shares_asset_id: p.shares_asset_id.parse()?,
            central_app_id: p.central_app_id.parse()?,
            slot_ids: to_slot_ids(&p.slot_ids)?,
            invest_escrow_address: p.invest_escrow_address.parse()?,
            staking_escrow_address: p.staking_escrow_address.parse()?,
            central_escrow_address: p.central_escrow_address.parse()?,
            customer_escrow_address: p.customer_escrow_address.parse()?,
            invest_link: p.invest_link,
            my_investment_link: p.my_investment_link,
            project_link: p.project_link,
            creator: p.creator_address.parse()?,
        })
    }
}

fn to_slot_ids(slot_ids_strs: &[String]) -> Result<Vec<u64>> {
    let mut slot_ids: Vec<u64> = vec![];
    for id_str in slot_ids_strs {
        slot_ids.push(id_str.parse()?);
    }
    Ok(slot_ids)
}
