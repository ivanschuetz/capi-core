#[cfg(test)]
pub use test::{
    create_and_distribute_funds_asset, setup_on_chain_deps, test_dao_init, test_dao_init_with_deps,
    test_dao_with_funds_target_init, test_dao_with_specs, test_init, OnChainDeps, TestDeps,
};

#[cfg(test)]
mod test {
    use crate::algo_helpers::{send_tx_and_wait, send_txs_and_wait};
    use crate::capi_deps::{CapiAddress, CapiAssetDaoDeps};
    use crate::dependencies::teal_api;
    use crate::files::{read_lines, write_to_file};
    use crate::flows::create_dao::setup_dao::Programs;
    use crate::flows::create_dao::setup_dao_specs::SetupDaoSpecs;
    use crate::teal::TealApi;
    use crate::testing::flow::create_dao_flow::test::test_programs;
    use crate::testing::test_data::{
        dao_specs, dao_specs_with_funds_target, funds_asset_creator, msig_acc1, msig_acc2,
        msig_acc3,
    };
    use crate::testing::tests_msig::TestsMsig;
    use algonaut::core::Address;
    use algonaut::{
        algod::v2::Algod,
        core::SuggestedTransactionParams,
        indexer::v2::Indexer,
        transaction::{
            account::Account, tx_group::TxGroup, AcceptAsset, CreateAsset, TransferAsset,
            TxnBuilder,
        },
    };
    use chrono::{Duration, Utc};
    use data_encoding::HEXLOWER;
    use mbase::date_util::DateTimeExt;
    use mbase::dependencies::{
        algod, algod_for_net, algod_for_tests, indexer_for_tests, network, DataType, Env, Network,
    };
    use mbase::models::asset_amount::AssetAmount;
    use mbase::models::funds::{FundsAmount, FundsAssetId};
    use mbase::models::shares_percentage::SharesPercentage;
    use rust_decimal::Decimal;
    use std::convert::TryInto;
    use std::str::FromStr;
    use tokio::test;
    use {
        crate::logger::init_logger,
        crate::testing::test_data::{capi_owner, creator, customer, investor1, investor2},
        crate::testing::TESTS_DEFAULT_PRECISION,
        anyhow::{anyhow, Result},
        dotenv::dotenv,
        std::env,
        std::process::Command,
        std::{
            io::{BufRead, BufReader},
            process::Stdio,
        },
    };

    pub struct TestDeps {
        pub algod: Algod,
        pub indexer: Indexer,
        pub api: Box<dyn TealApi>,

        pub creator: Account,
        pub investor1: Account,
        pub investor2: Account,
        pub customer: Account,

        pub specs: SetupDaoSpecs,

        pub msig: TestsMsig,

        pub funds_asset_id: FundsAssetId,

        pub capi_escrow_percentage: SharesPercentage,
        pub capi_address: CapiAddress,

        pub precision: u64,

        pub programs: Programs,
    }

    impl TestDeps {
        pub fn dao_deps(&self) -> CapiAssetDaoDeps {
            CapiAssetDaoDeps {
                escrow_percentage: self.capi_escrow_percentage,
                address: CapiAddress(capi_owner().address()),
            }
        }
    }

    fn msig() -> Result<TestsMsig> {
        Ok(TestsMsig::new(vec![msig_acc1(), msig_acc2(), msig_acc3()])?)
    }

    // TODO rename - it suggests that the dao is initialized here, which isn't the case - maybe crate_test_deps?
    /// Common tests initialization
    /// Guarantee: the returned funds raising end date is in the past and the target is 0,
    /// this means that the funds raising ended successfully,
    /// (which makes these deps backwards compatible with the pre-minfunds feature tests,
    /// where it's assumed that funds can always / unconditionally be withdrawn).
    pub async fn test_dao_init() -> Result<TestDeps> {
        test_dao_with_specs(&dao_specs()).await
    }

    /// Guarantee: the returned funds raising end date is in a week
    /// Relevant for test generally is that it's "later" by a safe span, so e.g. a withdrawal performed "now" with these deps will fail,
    /// as end date is in a week and withdrawals have to happen after it
    pub async fn test_dao_with_funds_target_init() -> Result<TestDeps> {
        // this needs to be dynamic, because we use a dynamic "now" reference date in TEAL and we've to ensure that this is after that
        // specifically 1 week doesn't have a particular reason, other than being a reasonable funding timeline generally
        let funds_end_date = Utc::now() + Duration::weeks(1);
        test_dao_with_specs(&dao_specs_with_funds_target(funds_end_date.to_timestap())).await
    }

    // named internal to not change the old "test_dao_init", which now means init without funds target specs
    pub async fn test_dao_with_specs(specs: &SetupDaoSpecs) -> Result<TestDeps> {
        test_init()?;

        let algod = algod_for_tests();
        let capi_owner = capi_owner();

        let chain_deps = setup_on_chain_deps(&algod, &capi_owner).await?;

        test_dao_init_with_deps(algod, &chain_deps, specs).await
    }

    /// Use this for test initialization with custom chain deps
    pub async fn test_dao_init_with_deps(
        algod: Algod,
        chain_deps: &OnChainDeps,
        specs: &SetupDaoSpecs,
    ) -> Result<TestDeps> {
        let OnChainDeps {
            funds_asset_id,
            capi_address,
        } = chain_deps;

        Ok(TestDeps {
            algod,
            indexer: indexer_for_tests(),
            api: Box::new(teal_api()),
            creator: creator(),
            investor1: investor1(),
            investor2: investor2(),
            customer: customer(),
            msig: msig()?,
            specs: specs.to_owned(),
            funds_asset_id: funds_asset_id.clone(),

            precision: TESTS_DEFAULT_PRECISION,

            capi_escrow_percentage: capi_escrow_percentage(),
            capi_address: capi_address.to_owned(),

            programs: test_programs()?,
        })
    }

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

    fn test_accounts_initial_funds() -> FundsAmount {
        FundsAmount::new(10_000_000_000)
    }

    pub async fn create_and_distribute_funds_asset(algod: &Algod) -> Result<FundsAssetId> {
        let params = algod.suggested_transaction_params().await?;

        let asset_creator = funds_asset_creator();
        let asset_id = create_funds_asset(algod, &params, &asset_creator).await?;

        // we want to only opt-in, not fund the capi owner. the capi owner is assumed to start without any funding
        // no reason other than backwards compatibility with tests
        optin_and_submit(algod, &params, asset_id.0, &capi_owner()).await?;

        let accounts = &[
            creator(),
            investor1(),
            investor2(),
            customer(),
            msig_acc1(),
            msig_acc2(),
            msig_acc3(),
        ];

        let initial_funds = test_accounts_initial_funds();

        // Log the funded addresses
        let addresses_str = accounts
            .into_iter()
            .map(|a| a.address().to_string())
            .collect::<Vec<String>>()
            .join(", ");
        log::debug!(
            "Funding accounts: {addresses_str} with: {} of funds asset: {}",
            initial_funds.0,
            asset_id.0
        );

        optin_and_fund_accounts_with_asset(
            algod,
            &params,
            asset_id.0,
            initial_funds,
            &asset_creator,
            accounts,
        )
        .await?;

        Ok(asset_id)
    }

    /// Creates the funds asset and capi dependencies
    pub async fn setup_on_chain_deps(algod: &Algod, capi_owner: &Account) -> Result<OnChainDeps> {
        let params = algod.suggested_transaction_params().await?;
        let funds_asset_id = create_and_distribute_funds_asset(algod).await?;

        optin_and_send_asset_to_msig(
            algod,
            &params,
            funds_asset_id.0,
            test_accounts_initial_funds().0,
            &funds_asset_creator(),
            &msig()?,
        )
        .await?;

        log::info!("funds_asset_id: {funds_asset_id:?}");

        Ok(OnChainDeps {
            funds_asset_id,
            capi_address: CapiAddress(capi_owner.address()),
        })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OnChainDeps {
        pub funds_asset_id: FundsAssetId,
        // the address to which the platform fees are directed
        pub capi_address: CapiAddress,
    }

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

        let signed_t = creator.sign_transaction(t)?;

        let p_tx = send_tx_and_wait(&algod, &signed_t).await?;
        let asset_id = p_tx
            .asset_index
            .ok_or_else(|| anyhow!("Couldn't retrieve asset id from pending tx"))?;

        log::info!("Created funds asset: {}", asset_id);

        Ok(FundsAssetId(asset_id))
    }

    async fn optin_and_fund_accounts_with_asset(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        asset_id: u64,
        amount: FundsAmount,
        sender: &Account,
        accounts: &[Account],
    ) -> Result<()> {
        for account in accounts {
            optin_and_send_asset_to_account(algod, params, asset_id, amount.0, sender, &account)
                .await?;
        }
        Ok(())
    }

    fn capi_escrow_percentage() -> SharesPercentage {
        // unwraps: hardcoded value, which we knows works + this is used only in tests
        Decimal::from_str("0.1").unwrap().try_into().unwrap()
    }

    async fn optin_and_submit(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        asset_id: u64,
        account: &Account,
    ) -> Result<()> {
        // optin the receiver to the asset
        let optin_tx = TxnBuilder::with(
            params,
            AcceptAsset::new(account.address(), asset_id).build(),
        )
        .build()?;

        let optin_tx_signed = account.sign_transaction(optin_tx)?;

        send_txs_and_wait(&algod, &[optin_tx_signed]).await?;

        log::debug!("Opted in: {}, to asset: {asset_id}", account.address());

        Ok(())
    }

    async fn optin_and_send_asset_to_account(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        asset_id: u64,
        amount: AssetAmount,
        sender: &Account,
        receiver: &Account,
    ) -> Result<()> {
        // optin the receiver to the asset
        let mut optin_tx = TxnBuilder::with(
            params,
            AcceptAsset::new(receiver.address(), asset_id).build(),
        )
        .build()?;

        let mut fund_tx = TxnBuilder::with(
            params,
            TransferAsset::new(sender.address(), asset_id, amount.0, receiver.address()).build(),
        )
        .build()?;

        TxGroup::assign_group_id(&mut [&mut optin_tx, &mut fund_tx])?;

        let optin_tx_signed = receiver.sign_transaction(optin_tx)?;
        let fund_tx_signed = sender.sign_transaction(fund_tx)?;

        send_txs_and_wait(&algod, &[optin_tx_signed, fund_tx_signed]).await?;

        log::debug!(
            "Opted in and funded: {}, asset: {asset_id}",
            receiver.address()
        );

        Ok(())
    }

    /// Note that sending of algos to the msig address is done in fund_accounts_sandbox.sh. This flow could be improved (TODO low prio)
    async fn optin_and_send_asset_to_msig(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        asset_id: u64,
        amount: AssetAmount,
        sender: &Account,
        receiver: &TestsMsig,
    ) -> Result<()> {
        // optin the receiver to the asset
        let mut optin_tx = TxnBuilder::with(
            params,
            AcceptAsset::new(receiver.address().address(), asset_id).build(),
        )
        .build()?;

        let mut fund_tx = TxnBuilder::with(
            params,
            TransferAsset::new(
                sender.address(),
                asset_id,
                amount.0,
                receiver.address().address(),
            )
            .build(),
        )
        .build()?;

        TxGroup::assign_group_id(&mut [&mut optin_tx, &mut fund_tx])?;

        let optin_tx_signed = receiver.sign(optin_tx)?;
        let fund_tx_signed = sender.sign_transaction(fund_tx)?;

        send_txs_and_wait(&algod, &[optin_tx_signed, fund_tx_signed]).await?;

        log::debug!(
            "Opted in and funded (funds asset): {}",
            receiver.address().address()
        );

        Ok(())
    }

    async fn send_asset_to_account(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        asset_id: u64,
        amount: AssetAmount,
        sender: &Account,
        receiver: &Address,
    ) -> Result<()> {
        let fund_tx = TxnBuilder::with(
            params,
            TransferAsset::new(sender.address(), asset_id, amount.0, *receiver).build(),
        )
        .build()?;
        let fund_tx_signed = sender.sign_transaction(fund_tx)?;
        send_tx_and_wait(&algod, &fund_tx_signed).await?;
        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TestInitResult {
        pub funds_asset_id: FundsAssetId,
    }

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
    #[test]
    #[ignore]
    async fn reset_and_fund_local_network() -> Result<()> {
        test_init()?;
        let deps = reset_and_fund_network(&Network::SandboxPrivate).await?;

        update_wasm_deps(
            &deps,
            WasmBuildConfig::Debug,
            &Network::SandboxPrivate,
            &Env::Local,
            &DataType::Real,
        )?;

        Ok(())
    }

    /// To be executed only once (unless it's required to re-create the dependencies)
    /// IMPORTANT: ensure that all the test accounts (found in test_data) are funded with Algos (using the dispenser) - these tests don't do this
    /// and will be interrupted if the accounts don't have enough funds to pay for setup fees.
    #[test]
    #[ignore]
    async fn reset_and_fund_testnet() -> Result<()> {
        init_logger()?;
        // Commented for safety - to prevent creating things on TestNet if running by mistake
        // let deps = reset_and_fund_network(&Network::Test).await?;
        // update_wasm_deps(&deps, WasmBuildConfig::Release, &Network::Test, &Env::Test)?;
        Ok(())
    }

    pub async fn reset_and_fund_network(net: &Network) -> Result<OnChainDeps> {
        let algod = algod_for_net(net);
        let capi_owner = capi_owner();

        let deps = setup_on_chain_deps(&algod, &capi_owner).await?;
        log::info!("Capi deps: {deps:?}");

        Ok(deps)
    }

    /////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////////////////
    /// NOTE: this should be in the WASM project - core shouldn't have any WASM dependencies. Temporary exception.
    /// WASM currently can't use tokio::test (for async tests)
    /// to fix this, we've to rename this project ("core" causes conflicts)
    /////////////////////////////////////////////////////////////

    // dead code: release config, usage commented
    #[allow(dead_code)]
    #[derive(Debug)]
    enum WasmBuildConfig {
        Debug,
        Release,
    }

    /// Updates the WASM project with generated local settings
    fn update_wasm_deps(
        deps: &OnChainDeps,
        build_config: WasmBuildConfig,
        network: &Network,
        env: &Env,
        data_type: &DataType,
    ) -> Result<()> {
        let build_config_str = match build_config {
            WasmBuildConfig::Debug => "debug",
            WasmBuildConfig::Release => "release",
        };

        let wasm_repo_path = "../frontend/wasm";
        let wasm_scrits_path = format!("{wasm_repo_path}/scripts");

        let wasm_local_build_script_path = format!("{wasm_scrits_path}/build_local.sh");

        let mut vars = generate_env_vars_for_config(network, env, data_type);
        let deps_vars = generate_env_vars_for_deps(deps);
        vars.extend(deps_vars);
        let vars_str = vars
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");

        let build_command = format!("wasm-pack build --out-dir ../wasm-build --{build_config_str}");
        let complete_build_command = format!("{vars_str} {build_command}");

        write_to_file(wasm_local_build_script_path, &complete_build_command)?;

        Ok(())
    }

    fn generate_env_vars_for_config(
        network: &Network,
        env: &Env,
        data_type: &DataType,
    ) -> Vec<(String, String)> {
        let network_str = match network {
            Network::SandboxPrivate => "sandbox_private",
            Network::Test => "test",
            Network::Private => "private",
        };
        let env_str = match env {
            Env::Test => "test",
            Env::Local => "local",
        };
        let data_type_str = match data_type {
            DataType::Real => "real",
            DataType::Mock => "mock",
        };
        vec![
            ("NETWORK".to_owned(), network_str.to_owned()),
            ("ENV".to_owned(), env_str.to_owned()),
            ("DATA_TYPE".to_owned(), data_type_str.to_owned()),
        ]
    }

    fn generate_env_vars_for_deps(deps: &OnChainDeps) -> Vec<(String, String)> {
        vec![
            (
                "FUNDS_ASSET_ID".to_owned(),
                deps.funds_asset_id.0.to_string(),
            ),
            ("CAPI_ADDRESS".to_owned(), deps.capi_address.0.to_string()),
        ]
    }

    /////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////////////////

    /// Run this to send some assets to an account, opting the account in to the asset
    #[test]
    #[ignore]
    async fn do_optin_and_fund_account_with_funds_asset() -> Result<()> {
        init_logger()?;

        let algod = algod_for_net(&Network::Test);

        let params = algod.suggested_transaction_params().await?;

        optin_and_send_asset_to_account(
            &algod,
            &params,
            75503403,
            AssetAmount(1_000_000_000),
            &capi_owner(),
            &Account::from_mnemonic("frame engage radio switch little scan time column amused spatial dynamic play cruise split coral aisle midnight cave essence midnight mutual dog ostrich absent leopard").unwrap(),
        )
        .await?;
        Ok(())
    }

    /// Run this to send some assets to an account
    #[test]
    #[ignore]
    async fn do_fund_account_with_funds_asset() -> Result<()> {
        init_logger()?;

        let algod = algod_for_net(&Network::Test);

        let params = algod.suggested_transaction_params().await?;

        send_asset_to_account(
            &algod,
            &params,
            75503403,
            AssetAmount(1_000_000_000),
            &capi_owner(),
            &"STOUDMINSIPP7JMJMGXVJYVS6HHD3TT5UODCDPYGV6KBGP7UYNTLJVJJME"
                .parse()
                .unwrap(),
        )
        .await?;
        Ok(())
    }

    /// Funds the accounts from the test accounts file with the funds asset.
    /// This gives us pre-funded accounts to share with testers for quick setup.
    #[test]
    #[ignore]
    async fn do_prepare_test_accounts() -> Result<()> {
        init_logger()?;

        let algod = algod_for_net(&Network::Test);

        let params = algod.suggested_transaction_params().await?;

        let accounts = load_test_accounts().await?;

        let assets_sender = capi_owner();

        // Funds asset
        optin_and_fund_accounts_with_asset(
            &algod,
            &params,
            81166440,
            FundsAmount::new(10_000_000_000),
            &assets_sender,
            &accounts,
        )
        .await?;

        Ok(())
    }

    async fn load_test_accounts() -> Result<Vec<Account>> {
        let mut accounts = vec![];
        for line_res in read_lines("./test_accounts.txt")? {
            let line = line_res?;
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("#") {
                let account = Account::from_mnemonic(trimmed)?;
                accounts.push(account)
            }
        }
        log::debug!("Loaded {} accounts", accounts.len());
        Ok(accounts)
    }

    #[test]
    #[ignore]
    async fn hex_to_address() {
        let hex = "8c025cac37c404934e0066f5407032a6e2294b54026ee39fcd272b23643f5916";
        let bytes = HEXLOWER.decode(hex.as_bytes()).unwrap();
        let address = Address(bytes.try_into().unwrap());
        println!("Hex: {} -> address: {}", hex, address);
    }

    #[test]
    #[ignore]
    async fn send_payment() -> Result<()> {
        let receiver = "OO7F7V6NG6BISF336ST4UVBTVMYNSG2BOOA3XKF5OBFP6LPMIJHRYWZRO4"
            .parse()
            .unwrap();
        let funds_asset_id = FundsAssetId(12);

        let amount = FundsAmount::new(100_000_000);

        let algod = algod(); // environment network - if using from WASM scripts, the net passed in build script
        let sender = funds_asset_creator(); // arbitrary account we know has enough assets

        let params = algod.suggested_transaction_params().await?;
        let tx = TxnBuilder::with(
            &params,
            TransferAsset::new(sender.address(), funds_asset_id.0, amount.val(), receiver).build(),
        )
        .build()?;
        let fund_tx_signed = sender.sign_transaction(tx)?;
        send_txs_and_wait(&algod, &[fund_tx_signed]).await?;

        println!("Funded!");

        Ok(())
    }
}
