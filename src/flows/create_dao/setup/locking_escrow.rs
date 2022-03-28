use algonaut::{
    algod::v2::Algod,
    core::{Address, MicroAlgos, SuggestedTransactionParams},
    transaction::{
        builder::TxnFee, contract_account::ContractAccount, AcceptAsset, Pay, SignedTransaction,
        Transaction, TxnBuilder,
    },
};
use anyhow::{anyhow, Result};
use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
use crate::teal::save_rendered_teal;
use crate::{
    algo_helpers::calculate_total_fee,
    api::version::{VersionedContractAccount, VersionedTealSourceTemplate},
    flows::create_dao::storage::load_dao::DaoAppId,
    teal::{render_template_new, TealSource, TealSourceTemplate},
};

// TODO no constant?
// 1 asset (funds asset)
const MIN_BALANCE: MicroAlgos = MicroAlgos(200_000);

async fn create_locking_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    source: &VersionedTealSourceTemplate,
    app_id: DaoAppId,
) -> Result<VersionedContractAccount> {
    render_and_compile_locking_escrow(algod, shares_asset_id, source, app_id).await
}

pub async fn render_and_compile_locking_escrow(
    algod: &Algod,
    shares_asset_id: u64,
    template: &VersionedTealSourceTemplate,
    app_id: DaoAppId,
) -> Result<VersionedContractAccount> {
    let source = match template.version.0 {
        1 => render_locking_escrow_v1(&template.template, shares_asset_id, app_id),
        _ => Err(anyhow!(
            "Locking escrow version not supported: {:?}",
            template.version
        )),
    }?;

    Ok(VersionedContractAccount {
        version: template.version,
        account: ContractAccount::new(algod.compile_teal(&source.0).await?),
    })
}

fn render_locking_escrow_v1(
    source: &TealSourceTemplate,
    shares_asset_id: u64,
    app_id: DaoAppId,
) -> Result<TealSource> {
    let escrow_source = render_template_new(
        source,
        &[
            ("TMPL_SHARES_ASSET_ID", &shares_asset_id.to_string()),
            ("TMPL_CENTRAL_APP_ID", &app_id.to_string()),
        ],
    )?;
    #[cfg(not(target_arch = "wasm32"))]
    save_rendered_teal("locking_escrow", escrow_source.clone())?; // debugging
    Ok(escrow_source)
}

pub async fn setup_locking_escrow_txs(
    algod: &Algod,
    source: &VersionedTealSourceTemplate,
    shares_asset_id: u64,
    creator: &Address,
    params: &SuggestedTransactionParams,
    app_id: DaoAppId,
) -> Result<SetupLockingEscrowToSign> {
    log::debug!(
        "Setting up escrow with asset id: {}, creator: {:?}",
        shares_asset_id,
        creator
    );

    let escrow = create_locking_escrow(algod, shares_asset_id, source, app_id).await?;
    log::debug!(
        "Generated locking escrow address: {:?}",
        *escrow.account.address()
    );

    // Send some funds to the escrow (min amount to hold asset, pay for opt in tx fee)
    let fund_algos_tx = &mut TxnBuilder::with(
        params,
        Pay::new(*creator, *escrow.account.address(), MIN_BALANCE).build(),
    )
    .build()?;

    let shares_optin_tx = &mut TxnBuilder::with_fee(
        params,
        TxnFee::zero(),
        AcceptAsset::new(*escrow.account.address(), shares_asset_id).build(),
    )
    .build()?;

    fund_algos_tx.fee = calculate_total_fee(params, &[fund_algos_tx, shares_optin_tx])?;

    Ok(SetupLockingEscrowToSign {
        escrow,
        escrow_shares_optin_tx: shares_optin_tx.clone(),
        escrow_funding_algos_tx: fund_algos_tx.clone(),
    })
}

pub async fn submit_locking_setup_escrow(
    algod: &Algod,
    signed: SetupLockingEscrowSigned,
) -> Result<SubmitSetupLockingEscrowRes> {
    let shares_optin_escrow_res = algod
        .broadcast_signed_transaction(&signed.shares_optin_tx)
        .await?;
    log::debug!("shares_optin_escrow_res: {:?}", shares_optin_escrow_res);

    Ok(SubmitSetupLockingEscrowRes {
        shares_optin_escrow_algos_tx_id: shares_optin_escrow_res.tx_id,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupLockingEscrowToSign {
    pub escrow: VersionedContractAccount,
    pub escrow_shares_optin_tx: Transaction,
    // min amount to hold asset (shares) + asset optin tx fee
    pub escrow_funding_algos_tx: Transaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupLockingEscrowSigned {
    pub escrow: ContractAccount,
    pub shares_optin_tx: SignedTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitSetupLockingEscrowRes {
    pub shares_optin_escrow_algos_tx_id: String,
}

#[derive(Serialize)]
struct EditTemplateContext {
    shares_asset_id: String,
    app_id: String,
}
