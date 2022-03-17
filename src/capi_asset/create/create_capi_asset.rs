use crate::{
    algo_helpers::send_tx_and_wait,
    capi_asset::capi_asset_id::{CapiAssetAmount, CapiAssetId},
};
use algonaut::{
    algod::v2::Algod,
    core::{Address, SuggestedTransactionParams},
    transaction::{CreateAsset, SignedTransaction, Transaction, TxnBuilder},
};
use anyhow::{anyhow, Result};

/// Creates the Capi asset, which entitles holders to a dividend of all the DAOs on the platform.
pub async fn create_capi_asset(
    supply: CapiAssetAmount,
    creator: &Address,
    params: &SuggestedTransactionParams,
) -> Result<CreateCapiAssetToSign> {
    let tx = TxnBuilder::with(
        params,
        CreateAsset::new(*creator, supply.val(), 0, false)
            // Should be called CAPI - for now using a different name to not attract attention on TestNet
            .unit_name("GLOB".to_owned())
            .asset_name("glob".to_owned())
            .build(),
    )
    .build()?;

    Ok(CreateCapiAssetToSign {
        create_capi_asset_tx: tx,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCapiAssetToSign {
    pub create_capi_asset_tx: Transaction,
}

pub async fn submit_create_capi_asset(
    algod: &Algod,
    create_shares: &SignedTransaction,
) -> Result<CreateCapiAssetResult> {
    let p_tx = send_tx_and_wait(algod, create_shares).await?;

    let asset_id = p_tx
        .asset_index
        .ok_or_else(|| anyhow!("Capi asset id in pending tx not set"))?;

    Ok(CreateCapiAssetResult {
        asset_id: CapiAssetId(asset_id),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCapiAssetResult {
    pub asset_id: CapiAssetId,
}
