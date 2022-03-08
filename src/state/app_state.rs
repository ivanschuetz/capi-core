use algonaut::{
    algod::v2::Algod,
    core::Address,
    error::AlgonautError,
    model::algod::v2::{Account, ApplicationLocalState, TealKeyValue, TealValue},
};
use anyhow::{anyhow, Error, Result};
use data_encoding::BASE64;
use std::{
    convert::TryInto,
    fmt::{self, Display, Formatter},
};

pub async fn global_state(algod: &Algod, app_id: u64) -> Result<ApplicationGlobalState> {
    let app = algod.application_information(app_id).await?;
    Ok(ApplicationGlobalState(app.params.global_state))
}

pub async fn local_state(
    algod: &Algod,
    address: &Address,
    app_id: u64,
) -> Result<ApplicationLocalState, ApplicationLocalStateError<'static>> {
    let investor_account_infos = algod.account_information(address).await?;
    local_state_from_account(&investor_account_infos, app_id)
}

pub fn local_state_from_account(
    account: &Account,
    app_id: u64,
) -> Result<ApplicationLocalState, ApplicationLocalStateError<'static>> {
    account
        .apps_local_state
        .iter()
        .find(|ls| ls.id == app_id)
        .cloned()
        .ok_or(ApplicationLocalStateError::NotOptedIn)
}

pub fn local_state_with_key(
    app_local_state: ApplicationLocalState,
    key: &AppStateKey,
) -> Option<TealValue> {
    find_value(&app_local_state.key_value, key)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationLocalStateError<'a> {
    NotOptedIn,
    LocalStateNotFound(AppStateKey<'a>),
    Msg(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppStateKey<'a>(pub &'a str);

/// Just a wrapper equivalent to ApplicationLocalState (provided by the SDK), to offer a similar interface
pub struct ApplicationGlobalState(Vec<TealKeyValue>);

pub trait ApplicationStateExt {
    fn find(&self, key: &AppStateKey) -> Option<TealValue>;
    fn find_uint(&self, key: &AppStateKey) -> Option<u64>;
    fn find_bytes(&self, key: &AppStateKey) -> Option<Vec<u8>>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ApplicationStateExt for ApplicationLocalState {
    fn find(&self, key: &AppStateKey) -> Option<TealValue> {
        find_value(&self.key_value, key)
    }

    fn find_uint(&self, key: &AppStateKey) -> Option<u64> {
        self.find(key).map(|kv| kv.uint)
    }

    fn find_bytes(&self, key: &AppStateKey) -> Option<Vec<u8>> {
        self.find(key).map(|kv| kv.bytes)
    }
    fn len(&self) -> usize {
        self.key_value.len()
    }
}

impl ApplicationStateExt for ApplicationGlobalState {
    fn find(&self, key: &AppStateKey) -> Option<TealValue> {
        find_value(&self.0, key)
    }

    fn find_uint(&self, key: &AppStateKey) -> Option<u64> {
        self.find(key).map(|kv| kv.uint)
    }

    fn find_bytes(&self, key: &AppStateKey) -> Option<Vec<u8>> {
        self.find(key).map(|kv| kv.bytes)
    }
    fn len(&self) -> usize {
        self.0.len()
    }
}

fn find_value(key_values: &[TealKeyValue], key: &AppStateKey) -> Option<TealValue> {
    key_values
        .iter()
        .find(|kv| kv.key_matches(key))
        .map(|kv| kv.value.clone())
}

trait TealKeyValueExt {
    fn key_matches(&self, key: &AppStateKey) -> bool;
}

impl TealKeyValueExt for TealKeyValue {
    fn key_matches(&self, key: &AppStateKey) -> bool {
        self.key == BASE64.encode(key.0.as_bytes())
    }
}

impl Display for ApplicationLocalStateError<'static> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<AlgonautError> for ApplicationLocalStateError<'static> {
    fn from(err: AlgonautError) -> Self {
        ApplicationLocalStateError::Msg(err.to_string())
    }
}

impl From<ApplicationLocalStateError<'static>> for anyhow::Error {
    fn from(err: ApplicationLocalStateError<'static>) -> Self {
        anyhow!("{}", err)
    }
}

pub fn get_uint_value_or_error(
    state: &ApplicationLocalState,
    key: &AppStateKey<'static>,
) -> Result<u64, ApplicationLocalStateError<'static>> {
    state
        .find_uint(key)
        .ok_or_else(|| ApplicationLocalStateError::LocalStateNotFound(key.to_owned()))
}

pub fn get_bytes_value_or_error(
    state: &ApplicationLocalState,
    key: &AppStateKey<'static>,
) -> Result<Vec<u8>, ApplicationLocalStateError<'static>> {
    state
        .find_bytes(key)
        .ok_or_else(|| ApplicationLocalStateError::LocalStateNotFound(key.to_owned()))
}

pub fn read_address_from_state(
    state: &dyn ApplicationStateExt,
    key: AppStateKey,
    log_identifier: &str,
) -> Result<Address> {
    let bytes = state.find_bytes(&key).ok_or(anyhow!(
        "Unexpected: {log_identifier} address not in global state"
    ))?;

    Ok(Address(bytes.try_into().map_err(|e| {
        Error::msg(format!(
            "Illegal state: couldn't convert {log_identifier} bytes to address: {e:?}"
        ))
    })?))
}
