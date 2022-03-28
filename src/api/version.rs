use crate::teal::TealSourceTemplate;
use algonaut::{
    core::Address,
    transaction::{
        contract_account::ContractAccount, error::TransactionError, SignedTransaction, Transaction,
    },
};
use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versions {
    pub app_approval: Version,
    pub app_clear: Version,
    pub central_escrow: Version,
    pub customer_escrow: Version,
    pub investing_escrow: Version,
    pub locking_escrow: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapiVersions {
    pub app_approval: Version,
    pub app_clear: Version,
    pub escrow: Version,
}

pub fn bytes_to_versions(state: &[u8]) -> Result<Versions> {
    let array: &[u8; 24] = state.try_into()?;
    bytes_array_to_versions(array)
}

pub fn bytes_array_to_versions(state: &[u8; 24]) -> Result<Versions> {
    // try_into() could be unwrap - this should always succeed, just being careful+
    Ok(Versions {
        app_approval: to_version(state[0..4].try_into()?),
        app_clear: to_version(state[4..8].try_into()?),
        central_escrow: to_version(state[8..12].try_into()?),
        customer_escrow: to_version(state[12..16].try_into()?),
        investing_escrow: to_version(state[16..20].try_into()?),
        locking_escrow: to_version(state[20..24].try_into()?),
    })
}

pub fn versions_to_bytes(versions: Versions) -> Result<Vec<u8>> {
    Ok(versions_to_bytes_array(versions)?.to_vec())
}

fn versions_to_bytes_array(versions: Versions) -> Result<[u8; 24]> {
    Ok([
        to_bytes(versions.app_approval),
        to_bytes(versions.app_clear),
        to_bytes(versions.central_escrow),
        to_bytes(versions.customer_escrow),
        to_bytes(versions.investing_escrow),
        to_bytes(versions.locking_escrow),
    ]
    .concat()
    // this should always succeed, just being careful+
    .try_into()
    .map_err(|_| Error::msg("Unexpected: couldn't convert version bytes to result array type"))?)
}

fn to_version(array: [u8; 4]) -> Version {
    Version(u32::from_le_bytes(array))
}

fn to_bytes(version: Version) -> [u8; 4] {
    version.0.to_le_bytes()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedAddress {
    pub version: Version,
    pub address: Address,
}

impl VersionedAddress {
    pub fn new(address: Address, version: Version) -> VersionedAddress {
        VersionedAddress { address, version }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedTealSourceTemplate {
    pub version: Version,
    pub template: TealSourceTemplate,
}

impl VersionedTealSourceTemplate {
    pub fn new(template: TealSourceTemplate, version: Version) -> VersionedTealSourceTemplate {
        VersionedTealSourceTemplate { template, version }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedContractAccount {
    pub version: Version,
    pub account: ContractAccount,
}

impl VersionedContractAccount {
    pub fn to_versioned_address(&self) -> VersionedAddress {
        VersionedAddress {
            version: self.version,
            address: *self.account.address(),
        }
    }

    pub fn address(&self) -> &Address {
        self.account.address()
    }

    pub fn sign(
        &self,
        transaction: &Transaction,
        args: Vec<Vec<u8>>,
    ) -> Result<SignedTransaction, TransactionError> {
        self.account.sign(transaction, args)
    }
}
