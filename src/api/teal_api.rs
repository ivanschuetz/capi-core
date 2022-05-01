use super::{
    contract::Contract,
    version::{Version, VersionedTealSourceTemplate, Versions},
};
use crate::teal::load_teal_template;
use anyhow::{anyhow, Result};

// Send + sync assumess the implementations to be stateless
// (also: we currently use this only in WASM, which is single threaded)
pub trait TealApi: Send + Sync {
    fn last_versions(&self) -> Versions;
    fn template(&self, contract: Contract, version: Version)
        -> Result<VersionedTealSourceTemplate>;
}

pub struct LocalTealApi {}

impl TealApi for LocalTealApi {
    fn last_versions(&self) -> Versions {
        Versions {
            app_approval: Version(1),
            app_clear: Version(1),
            customer_escrow: Version(1),
        }
    }

    fn template(
        &self,
        contract: Contract,
        version: Version,
    ) -> Result<VersionedTealSourceTemplate> {
        match contract {
            Contract::DaoCustomer => dao_customer_teal(version),
            Contract::DaoAppApproval => dao_app_approval_teal(version),
            Contract::DaoAppClear => dao_app_clear_teal(version),
            Contract::CapiAppApproval => capi_app_approval_teal(version),
            Contract::CapiAppClear => capi_app_clear_teal(version),
        }
    }
}

fn dao_customer_teal(version: Version) -> Result<VersionedTealSourceTemplate> {
    match version.0 {
        1 => load_versioned_teal_template(version, "customer_escrow"),
        _ => not_found_err("dao customer", version),
    }
}

fn dao_app_approval_teal(version: Version) -> Result<VersionedTealSourceTemplate> {
    match version.0 {
        1 => load_versioned_teal_template(version, "dao_app_approval"),
        _ => not_found_err("dao app", version),
    }
}

fn dao_app_clear_teal(version: Version) -> Result<VersionedTealSourceTemplate> {
    match version.0 {
        1 => load_versioned_teal_template(version, "dao_app_clear"),
        _ => not_found_err("dao app", version),
    }
}

fn capi_app_approval_teal(version: Version) -> Result<VersionedTealSourceTemplate> {
    match version.0 {
        1 => load_versioned_teal_template(version, "capi_app_approval"),
        _ => not_found_err("capi approval app", version),
    }
}

fn capi_app_clear_teal(version: Version) -> Result<VersionedTealSourceTemplate> {
    match version.0 {
        1 => load_versioned_teal_template(version, "capi_app_clear"),
        _ => not_found_err("capi clear app", version),
    }
}

fn load_versioned_teal_template(
    version: Version,
    file_name: &str,
) -> Result<VersionedTealSourceTemplate> {
    Ok(VersionedTealSourceTemplate {
        version,
        template: load_teal_template(file_name)?,
    })
}

fn not_found_err<T>(id: &str, version: Version) -> Result<T> {
    Err(anyhow!("Not found version: {version:?} for: {id}"))
}
