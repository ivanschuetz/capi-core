use algonaut::core::Address;
use mbase::models::shares_percentage::SharesPercentage;

/// Capi asset environment relevant to the DAOs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapiAssetDaoDeps {
    // Shares percentage is slightly wrong semantically: we just want a [0..1] percentage. For now repurposing.
    pub escrow_percentage: SharesPercentage,
    pub address: CapiAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapiAddress(pub Address);
