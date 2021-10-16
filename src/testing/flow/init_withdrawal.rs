#[cfg(test)]
use crate::flows::withdraw::init_withdrawal::{
    init_withdrawal, submit_init_withdrawal, InitWithdrawalSigned,
};
#[cfg(test)]
use algonaut::{algod::v2::Algod, core::MicroAlgos, transaction::account::Account};
#[cfg(test)]
use anyhow::Result;

#[cfg(test)]
pub async fn init_withdrawal_flow(
    algod: &Algod,
    creator: &Account,
    amount_to_withdraw: MicroAlgos,
    slot_id: u64,
) -> Result<String> {
    let to_sign = init_withdrawal(&algod, &creator.address(), amount_to_withdraw, slot_id).await?;

    // UI
    let signed = InitWithdrawalSigned {
        init_withdrawal_slot_app_call_tx: creator
            .sign_transaction(&to_sign.init_withdrawal_slot_app_call_tx)?,
    };

    Ok(submit_init_withdrawal(&algod, &signed).await?)
}
