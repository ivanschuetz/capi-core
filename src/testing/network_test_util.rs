#[cfg(test)]
use crate::capi_asset::{
    capi_asset_dao_specs::CapiAssetDaoDeps, capi_asset_id::CapiAssetAmount,
    create::test_flow::test_flow::setup_capi_asset_flow,
};
#[cfg(test)]
use crate::dependencies::algod_for_net;
#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use algonaut::{
    algod::v2::Algod,
    core::SuggestedTransactionParams,
    transaction::{
        account::Account, tx_group::TxGroup, AcceptAsset, CreateAsset, TransferAsset, TxnBuilder,
    },
};

#[cfg(test)]
use rust_decimal::Decimal;
#[cfg(test)]
use std::convert::TryInto;
#[cfg(test)]
use std::str::FromStr;
#[cfg(test)]
use tokio::test;
#[cfg(test)]
use {
    crate::dependencies::{network, Network},
    crate::flows::create_project::shares_percentage::SharesPercentage,
    crate::funds::{FundsAmount, FundsAssetId},
    crate::logger::init_logger,
    crate::testing::test_data::{capi_owner, creator, customer, investor1, investor2},
    anyhow::{anyhow, Result},
    dotenv::dotenv,
    std::env,
    std::process::Command,
    std::{
        io::{BufRead, BufReader},
        process::Stdio,
    },
};

/// Common tests initialization
#[cfg(test)]
pub fn test_init() -> Result<()> {
    // load vars in .env file

    dotenv().ok();

    if env::var("TESTS_LOGGING")?.parse::<i32>()? == 1 {
        init_logger()?;
        log::debug!("Logging is enabled");
    }
    reset_network(&network())?;

    Ok(())
}

#[cfg(test)]
pub async fn create_and_distribute_funds_asset(algod: &Algod) -> Result<FundsAssetId> {
    let params = algod.suggested_transaction_params().await?;

    // address: NIKGABIQLRCPJYCNCFZWR7GUIC3NA66EBVR65JKHKLGLIYQ4KO3YYPV67Q
    let asset_creator = Account::from_mnemonic("accident inherit artist kid such wheat sure then skirt horse afford penalty grant airport school aim hollow position ask churn extend soft mean absorb achieve")?;
    let asset_id = create_funds_asset(algod, &params, &asset_creator).await?;

    fund_accounts_with_local_funds_asset(
        algod,
        &params,
        asset_id,
        FundsAmount::new(10_000_000_000),
        &asset_creator,
    )
    .await?;
    Ok(asset_id)
}

/// Creates the funds asset and capi-token related dependencies
#[cfg(test)]
pub async fn setup_on_chain_deps(algod: &Algod) -> Result<OnChainDeps> {
    let funds_asset_id = create_and_distribute_funds_asset(algod).await?;
    let capi_deps = create_capi_asset_and_deps(algod, funds_asset_id).await?;

    Ok(OnChainDeps {
        funds_asset_id,
        capi_deps,
    })
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChainDeps {
    pub funds_asset_id: FundsAssetId,
    pub capi_deps: CapiAssetDaoDeps,
}

#[cfg(test)]
async fn create_funds_asset(
    algod: &Algod,
    params: &SuggestedTransactionParams,
    creator: &Account,
) -> Result<FundsAssetId> {
    let t = TxnBuilder::with(
        params,
        // 10 quintillions
        CreateAsset::new(creator.address(), 10_000_000_000_000_000_000, 6, false)
            .unit_name("TEST".to_owned())
            .asset_name("Test".to_owned())
            .build(),
    )
    .build()?;

    // we need to sign the transaction to prove that we own the sender address
    let signed_t = creator.sign_transaction(&t)?;

    // broadcast the transaction to the network

    let send_response = algod.broadcast_signed_transaction(&signed_t).await?;
    println!("Transaction ID: {}", send_response.tx_id);

    let pending_t = wait_for_pending_transaction(&algod, &send_response.tx_id.parse()?).await?;

    let asset_id = pending_t
        .and_then(|t| t.asset_index)
        .ok_or_else(|| anyhow!("Couldn't retrieve asset id from pending tx"))?;

    log::info!("Created funds asset: {}", asset_id);

    Ok(FundsAssetId(asset_id))
}

#[cfg(test)]
async fn fund_accounts_with_local_funds_asset(
    algod: &Algod,
    params: &SuggestedTransactionParams,
    funds_asset_id: FundsAssetId,
    amount: FundsAmount,
    sender: &Account,
) -> Result<()> {
    for account in vec![creator(), investor1(), investor2(), customer()] {
        fund_account_with_local_funds_asset(
            algod,
            params,
            funds_asset_id,
            amount,
            sender,
            &account,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
async fn create_capi_asset_and_deps(
    algod: &Algod,
    funds_asset_id: FundsAssetId,
) -> Result<CapiAssetDaoDeps> {
    let capi_owner = capi_owner();
    let capi_supply = CapiAssetAmount::new(1_000_000_000);

    let flow_res = setup_capi_asset_flow(&algod, &capi_owner, capi_supply, funds_asset_id).await?;

    Ok(CapiAssetDaoDeps {
        escrow: *flow_res.escrow.address(),
        escrow_percentage: capi_escrow_percentage(),
        app_id: flow_res.app_id,
    })
}

#[cfg(test)]
fn capi_escrow_percentage() -> SharesPercentage {
    // unwraps: hardcoded value, which we knows works + this is used only in tests
    Decimal::from_str("0.1").unwrap().try_into().unwrap()
}

#[cfg(test)]
async fn fund_account_with_local_funds_asset(
    algod: &Algod,
    params: &SuggestedTransactionParams,
    funds_asset_id: FundsAssetId,
    amount: FundsAmount,
    sender: &Account,
    receiver: &Account,
) -> Result<()> {
    // optin the receiver to the asset
    let optin_tx = &mut TxnBuilder::with(
        params,
        AcceptAsset::new(receiver.address(), funds_asset_id.0).build(),
    )
    .build()?;

    let fund_tx = &mut TxnBuilder::with(
        params,
        TransferAsset::new(
            sender.address(),
            funds_asset_id.0,
            amount.val(),
            receiver.address(),
        )
        .build(),
    )
    .build()?;

    TxGroup::assign_group_id(vec![optin_tx, fund_tx])?;

    let optin_tx_signed = receiver.sign_transaction(&optin_tx)?;
    let fund_tx_signed = sender.sign_transaction(&fund_tx)?;

    let res = algod
        .broadcast_signed_transactions(&[optin_tx_signed, fund_tx_signed])
        .await?;

    wait_for_pending_transaction(&algod, &res.tx_id.parse()?).await?;

    Ok(())
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestInitResult {
    pub funds_asset_id: FundsAssetId,
}

#[cfg(test)]
fn reset_network(net: &Network) -> Result<()> {
    let mut cmd = Command::new("sh");

    let cmd_with_net_args = match net {
        &Network::SandboxPrivate => cmd
            .current_dir("scripts/sandbox")
            .arg("./sandbox_reset_for_tests.sh"),
        Network::Private => cmd
            .current_dir("scripts/private_net")
            .arg("./private_net_reset_for_tests.sh"),
        Network::Test => panic!("Not supported: reseting testnet"),
    };

    let reset_res = cmd_with_net_args
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .expect("Couldn't reset network");

    for _line in BufReader::new(reset_res)
        .lines()
        .filter_map(|line| line.ok())
    {
        // log::debug!("{}", _line);
    }

    Ok(())
}

/// Reset and prepare local network for manual testing.
/// Basically execute the same code we do when starting all the automated tests.
#[test]
#[ignore]
async fn reset_and_fund_local_network() -> Result<()> {
    test_init()?;
    reset_and_fund_network(&Network::SandboxPrivate).await
}

/// To be executed only once (unless it's required to re-create the dependencies)
#[test]
#[ignore]
async fn reset_and_fund_testnet() -> Result<()> {
    init_logger()?;
    reset_and_fund_network(&Network::Test).await
}

#[cfg(test)]
async fn reset_and_fund_network(net: &Network) -> Result<()> {
    let algod = algod_for_net(net);
    let deps = setup_on_chain_deps(&algod).await?;
    log::info!("Capi deps: {deps:?}");

    Ok(())
}
