use crate::{
    capi_asset::capi_asset_dao_specs::CapiAssetDaoDeps,
    flows::create_project::{
        create_project::Escrows,
        create_project_specs::CreateProjectSpecs,
        model::{CreateSharesSpecs, Project},
        setup::{
            central_escrow::render_and_compile_central_escrow,
            customer_escrow::render_and_compile_customer_escrow,
            investing_escrow::render_and_compile_investing_escrow,
            locking_escrow::render_and_compile_locking_escrow,
        },
        share_amount::ShareAmount,
    },
    funds::{FundsAmount, FundsAssetId},
    hashable::Hashable,
};
use algonaut::{algod::v2::Algod, core::Address, crypto::HashDigest};
use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use futures::join;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::convert::TryInto;

pub fn project_to_note(project: &Project) -> Result<Vec<u8>> {
    let version_bytes = u16::to_be_bytes(1);

    let project_hash = project.hash()?;

    let project_note_payload: ProjectNoteProjectPayload = project.to_owned().into();
    let project_note_payload_bytes = project_note_payload.bytes()?;

    // Note that the hash belongs to the Project instance, not the saved payload.
    // This allows us to store a minimal representation and validate the generated full Project against the hash.
    // In this case minimal means that the programs (escrows) are not stored: they can be rendered on demand.
    let bytes = [
        version_bytes.as_slice(),
        &project_hash.0,
        &project_note_payload_bytes,
    ]
    .concat();
    Ok(bytes)
}

pub async fn base64_note_to_project(
    algod: &Algod,
    escrows: &Escrows,
    note: &str,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Project> {
    let bytes = BASE64.decode(note.as_bytes())?;
    note_to_project(algod, escrows, &bytes, capi_deps).await
}

async fn note_to_project(
    algod: &Algod,
    escrows: &Escrows,
    note: &[u8],
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Project> {
    let payload = note_to_project_payload(note)?;
    if payload.version != 1 {
        return Err(anyhow!(
            "Not supported project version in note: {}",
            payload.version
        ));
    }

    let variable = payload.variable;

    // The hash seems meaningless now that we're fetching the project data using the tx id (instead of the hash)
    // but we'll keep it for now. It doesn't hurt.
    let hashed_stored_project = extract_hash_and_object_from_note_payload(&variable)?;
    let stored_project = hashed_stored_project.obj;
    let stored_project_digest = hashed_stored_project.hash;

    let project = storable_project_to_project(
        algod,
        &stored_project,
        &stored_project_digest,
        escrows,
        capi_deps,
    )
    .await?;

    Ok(project)
}

fn note_to_project_payload(note: &[u8]) -> Result<ProjectPayload> {
    let version_bytes = note
        .get(0..2)
        .ok_or_else(|| anyhow!("Not enough bytes in note to get version. Note: {:?}", note))?;
    let version = u16::from_be_bytes(version_bytes.try_into()?);

    let variable_bytes = note
        .get(2..note.len())
        .ok_or_else(|| anyhow!("Not enough bytes in note to get version. Note: {:?}", note))?;

    Ok(ProjectPayload {
        version,
        variable: variable_bytes.to_vec(),
    })
}

fn extract_hash_and_object_from_note_payload<T>(payload: &[u8]) -> Result<ObjectAndHash<T>>
where
    T: DeserializeOwned,
{
    let hash_bytes = payload
        .get(0..32)
        .ok_or_else(|| anyhow!("Not enough bytes in note to get hash. Note: {:?}", payload))?;
    let hash = HashDigest(hash_bytes.try_into()?);

    let hashed_obj = payload.get(32..payload.len()).ok_or_else(|| {
        anyhow!(
            "Not enough bytes in note to get hashed object. Note: {:?}",
            payload
        )
    })?;

    let res = rmp_serde::from_slice(hashed_obj).map_err(|e| {
        anyhow!(
            "Failed deserializing hashed object bytes: {:?}, error: {}",
            hashed_obj,
            e
        )
    })?;

    Ok(ObjectAndHash { hash, obj: res })
}

#[derive(Debug, Clone)]
pub struct ObjectAndHash<T>
where
    T: DeserializeOwned,
{
    pub hash: HashDigest,
    pub obj: T,
}

/// Converts and completes the project data stored in note to a full project instance.
/// It also verifies the hash.
async fn storable_project_to_project(
    algod: &Algod,
    payload: &ProjectNoteProjectPayload,
    prefix_hash: &HashDigest,
    escrows: &Escrows,
    capi_deps: &CapiAssetDaoDeps,
) -> Result<Project> {
    // Render and compile the escrows
    let central_escrow_account_fut = render_and_compile_central_escrow(
        algod,
        &payload.creator,
        &escrows.central_escrow,
        payload.funds_asset_id,
        payload.central_app_id,
    );
    let locking_escrow_account_fut = render_and_compile_locking_escrow(
        algod,
        payload.shares_asset_id,
        &escrows.locking_escrow,
        payload.central_app_id,
    );

    let (central_escrow_account_res, locking_escrow_account_res) =
        join!(central_escrow_account_fut, locking_escrow_account_fut);
    let central_escrow_account = central_escrow_account_res?;
    let locking_escrow_account = locking_escrow_account_res?;

    let customer_escrow_account_fut = render_and_compile_customer_escrow(
        algod,
        central_escrow_account.address(),
        &escrows.customer_escrow,
        &capi_deps.escrow,
        payload.central_app_id,
    );

    let investing_escrow_account_fut = render_and_compile_investing_escrow(
        algod,
        payload.shares_asset_id,
        &payload.share_price,
        &payload.funds_asset_id,
        locking_escrow_account.address(),
        central_escrow_account.address(),
        &escrows.invest_escrow,
        payload.central_app_id,
    );

    let (customer_escrow_account_res, investing_escrow_account_res) =
        join!(customer_escrow_account_fut, investing_escrow_account_fut);
    let customer_escrow_account = customer_escrow_account_res?;
    let investing_escrow_account = investing_escrow_account_res?;

    let project = Project {
        specs: CreateProjectSpecs::new(
            payload.name.clone(),
            payload.description.clone(),
            CreateSharesSpecs {
                token_name: payload.asset_name.clone(),
                supply: payload.asset_supply,
            },
            payload.investors_part,
            payload.share_price,
            payload.logo_url.clone(),
            payload.social_media_url.clone(),
        )?,
        funds_asset_id: payload.funds_asset_id,
        creator: payload.creator,
        shares_asset_id: payload.shares_asset_id,
        central_app_id: payload.central_app_id,
        invest_escrow: investing_escrow_account,
        locking_escrow: locking_escrow_account,
        central_escrow: central_escrow_account,
        customer_escrow: customer_escrow_account,
    };

    // Verify hash (compare freshly calculated hash with prefix hash contained in note)
    // NOTE that this doesn't seem necessary anymore, as we're not using the prefix hash anymore to fetch (but the tx id instead)
    // but, why not: more verifications is better than less, if they don't impact significantly performance
    let hash = *project.compute_hash()?.hash();
    if &hash != prefix_hash {
        return Err(anyhow!(
            "Hash verification failed: Note hash doesn't correspond to calculated hash"
        ));
    }

    Ok(project)
}

/// The project representation that's directly stored in the storage tx note
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectNoteProjectPayload {
    pub name: String,
    pub description: String,
    pub asset_id: u64,

    // NOTE: asset name and supply are redundant, we save them to not have to fetch the asset infos (also, they're short).
    // When someone shares their project, people should be able to see it as quickly as possible.
    // Note also that these asset properties are immutable (https://developer.algorand.org/docs/get-details/asa/#modifying-an-asset), so it's safe to save them.
    pub asset_name: String,
    pub asset_supply: ShareAmount,

    pub share_price: FundsAmount,
    pub funds_asset_id: FundsAssetId,
    pub investors_part: ShareAmount,
    pub logo_url: String,
    pub social_media_url: String,
    pub creator: Address,
    pub shares_asset_id: u64,
    pub central_app_id: u64,
}

impl ProjectNoteProjectPayload {
    pub fn bytes(&self) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(self)?)
    }
}

impl From<Project> for ProjectNoteProjectPayload {
    fn from(p: Project) -> Self {
        ProjectNoteProjectPayload {
            name: p.specs.name.clone(),
            description: p.specs.description.clone(),
            social_media_url: p.specs.social_media_url.clone(),
            asset_id: p.shares_asset_id,
            asset_name: p.specs.shares.token_name.clone(),
            asset_supply: p.specs.shares.supply,
            funds_asset_id: p.funds_asset_id,
            share_price: p.specs.share_price,
            investors_part: p.specs.investors_part(),
            logo_url: p.specs.logo_url,
            creator: p.creator,
            shares_asset_id: p.shares_asset_id,
            central_app_id: p.central_app_id,
        }
    }
}

#[derive(Debug, Clone)]
struct ProjectPayload {
    version: u16,
    variable: Vec<u8>,
}
