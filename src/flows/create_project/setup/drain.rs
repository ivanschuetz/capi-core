use crate::teal::TealSourceTemplate;

use super::{
    central_escrow::{setup_central_escrow, SetupCentralEscrowToSign},
    customer_escrow::{setup_customer_escrow, SetupCustomerEscrowToSign},
};
use algonaut::{algod::v2::Algod, core::Address};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainSetupToSign {
    pub central: SetupCentralEscrowToSign,
    pub customer: SetupCustomerEscrowToSign,
}

/// Customer payments -> central -> % to investors
pub async fn setup_drain(
    algod: &Algod,
    central_escrow_source: TealSourceTemplate,
    customer_escrow_source: TealSourceTemplate,
    project_creator: &Address,
) -> Result<DrainSetupToSign> {
    let central_to_sign =
        setup_central_escrow(algod, project_creator, central_escrow_source).await?;

    let customer_to_sign = setup_customer_escrow(
        algod,
        project_creator,
        central_to_sign.escrow.address,
        customer_escrow_source,
    )
    .await?;

    Ok(DrainSetupToSign {
        central: central_to_sign,
        customer: customer_to_sign,
    })
}
