use crate::flows::create_dao::model::Dao;
use algonaut::algod::v2::Algod;
use anyhow::Result;
use mbase::{
    models::{dao_id::DaoId, share_amount::ShareAmount},
    state::dao_app_state::dao_global_state,
};

/// NOTE: this is an expensive function:
/// - Call to load dao app state
/// - Calls to retrieve TEAL templates for ALL the escrows (currently local, later this will come from API. Can be parallelized.)
/// - Call to retrieve asset information (supply etc, using the asset id stored in the app state)
/// - Calls to render and compile ALL the escrows (parallelized - 2 batches)
/// TODO parallelize more (and outside of this function, try to cache the dao, etc. to not have to call this often)
pub async fn load_dao(algod: &Algod, dao_id: DaoId) -> Result<Dao> {
    let app_id = dao_id.0;

    log::debug!("Fetching dao with id: {:?}", app_id);

    let dao_state = dao_global_state(algod, app_id).await?;

    // TODO store this state (redundantly in the same app field), to prevent this call?
    let asset_infos = algod.asset_information(dao_state.shares_asset_id).await?;

    let dao = Dao {
        funds_asset_id: dao_state.funds_asset_id,
        owner: dao_state.owner,
        shares_asset_id: dao_state.shares_asset_id,
        app_id,

        name: dao_state.project_name.clone(),
        descr_url: dao_state.project_desc_url.clone(),
        token_name: asset_infos.params.name.unwrap_or_else(|| "".to_owned()),
        token_supply: ShareAmount::new(asset_infos.params.total),
        investors_share: dao_state.investors_share,
        share_price: dao_state.share_price,
        image_nft: dao_state.image_nft.clone(),
        social_media_url: dao_state.social_media_url.clone(),
        raise_end_date: dao_state.min_funds_target_end_date,
        raise_min_target: dao_state.min_funds_target,
        raised: dao_state.raised,
        setup_date: dao_state.setup_date,
        prospectus: dao_state.prospectus.clone(),
        min_invest_amount: dao_state.min_invest_amount,
        max_invest_amount: dao_state.max_invest_amount,
    };

    Ok(dao)
}
