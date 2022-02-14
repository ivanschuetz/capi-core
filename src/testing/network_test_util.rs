#[cfg(test)]
use crate::network_util::wait_for_pending_transaction;
#[cfg(test)]
use algonaut::{
    algod::v2::Algod,
    core::SuggestedTransactionParams,
    transaction::{account::Account, CreateAsset, TransferAsset, TxnBuilder},
};
#[cfg(test)]
use {
    crate::dependencies::{network, Network},
    crate::funds::{FundsAmount, FundsAssetId},
    crate::logger::init_logger,
    crate::testing::test_data::{creator, customer, investor1, investor2},
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
    // address: DNQPINWK4K5QZYLCK7DVJFEWRUXPXGW36TEUIHNSNOFYI2RMPG2BZPQ7DE
    let asset_creator = Account::from_mnemonic("champion slab oyster plug add neutral gap burger civil gossip hybrid return truth mad light edit invest hybrid mistake allow flip quarter guess abstract ginger")?;
    let asset_id = create_funds_asset(algod, &params, &asset_creator).await?;
    fund_accounts_with_local_funds_asset(
        algod,
        &params,
        asset_id,
        FundsAmount(10_000_000_000),
        &asset_creator,
    )
    .await?;
    Ok(asset_id)
}

#[cfg(test)]
async fn create_funds_asset(
    algod: &Algod,
    params: &SuggestedTransactionParams,
    creator: &Account,
) -> Result<FundsAssetId> {
    let t = TxnBuilder::with(
        params.to_owned(),
        // 10 quintillions
        CreateAsset::new(creator.address(), 10_000_000_000_000_000_000, 6, false)
            .unit_name("TEST".to_owned())
            .asset_name("Local test funds asset".to_owned())
            .build(),
    )
    .build();

    // we need to sign the transaction to prove that we own the sender address
    let signed_t = creator.sign_transaction(&t)?;

    // broadcast the transaction to the network

    let send_response = algod.broadcast_signed_transaction(&signed_t).await?;
    println!("Transaction ID: {}", send_response.tx_id);

    let pending_t = wait_for_pending_transaction(&algod, &send_response.tx_id.parse()?).await?;

    let asset_id = pending_t
        .and_then(|t| t.asset_index)
        .ok_or_else(|| anyhow!("Couldn't retrieve asset id from pending tx"))?;

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
async fn fund_account_with_local_funds_asset(
    algod: &Algod,
    params: &SuggestedTransactionParams,
    funds_asset_id: FundsAssetId,
    amount: FundsAmount,
    sender: &Account,
    receiver: &Account,
) -> Result<()> {
    use algonaut::transaction::{tx_group::TxGroup, AcceptAsset};

    // optin the receiver to the asset
    let optin_tx = &mut TxnBuilder::with(
        params.to_owned(),
        AcceptAsset::new(receiver.address(), funds_asset_id.0).build(),
    )
    .build();

    let fund_tx = &mut TxnBuilder::with(
        params.clone(),
        TransferAsset::new(
            sender.address(),
            funds_asset_id.0,
            amount.0,
            receiver.address(),
        )
        .build(),
    )
    .build();

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
