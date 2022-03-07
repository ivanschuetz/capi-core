use algonaut::core::Address;

use crate::flows::create_project::shares_percentage::SharesPercentage;

use super::{capi_app_id::CapiAppId, capi_asset_id::CapiAssetId};

/// Capi asset environment relevant to the DAOs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapiAssetDaoDeps {
    pub escrow: Address,
    // Shares percentage is slightly wrong semantically: we just want a [0..1] percentage. For now repurposing.
    pub escrow_percentage: SharesPercentage,
    pub app_id: CapiAppId,
    pub asset_id: CapiAssetId,
}
