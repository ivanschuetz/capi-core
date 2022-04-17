#[cfg(test)]
pub use test::{
    create_and_distribute_funds_asset, setup_on_chain_deps, test_dao_init, test_dao_init_with_deps,
    test_init, OnChainDeps, TestDeps,
};

#[cfg(test)]
mod test {
    use crate::algo_helpers::{send_tx_and_wait, send_txs_and_wait};
    use crate::api::api::{Api, LocalApi};
    use crate::api::version::VersionedContractAccount;
    use crate::asset_amount::AssetAmount;
    use crate::capi_asset::capi_app_id::CapiAppId;
    use crate::capi_asset::capi_asset_id::CapiAssetId;
    use crate::capi_asset::create::test_flow::test_flow::{
        setup_and_submit_capi_escrow, CapiAssetFlowRes,
    };
    use crate::capi_asset::{
        capi_asset_dao_specs::CapiAssetDaoDeps, capi_asset_id::CapiAssetAmount,
        create::test_flow::test_flow::setup_capi_asset_flow,
    };
    use crate::dependencies::{algod_for_net, Env};
    use crate::files::{read_lines, write_to_file};
    use crate::flows::create_dao::create_dao::Programs;
    use crate::flows::create_dao::create_dao_specs::CreateDaoSpecs;
    use crate::testing::flow::create_dao_flow::test::{test_capi_programs, test_programs};
    use crate::testing::test_data::{dao_specs, msig_acc1, msig_acc2, msig_acc3};
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
    use data_encoding::HEXLOWER;
    use rust_decimal::Decimal;
    use std::convert::TryInto;
    use std::str::FromStr;
    use tokio::test;
    use {
        crate::dependencies::{self, network, Network},
        crate::flows::create_dao::shares_percentage::SharesPercentage,
        crate::funds::{FundsAmount, FundsAssetId},
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
        pub api: Box<dyn Api>,

        pub creator: Account,
        pub investor1: Account,
        pub investor2: Account,
        pub customer: Account,

        pub specs: CreateDaoSpecs,

        pub msig: TestsMsig,

        pub funds_asset_id: FundsAssetId,

        pub capi_owner: Account,
        pub capi_escrow: VersionedContractAccount,
        pub capi_escrow_percentage: SharesPercentage,
        pub capi_app_id: CapiAppId,
        pub capi_asset_id: CapiAssetId,
        pub capi_supply: CapiAssetAmount,

        pub precision: u64,

        pub programs: Programs,
    }

    impl TestDeps {
        pub fn dao_deps(&self) -> CapiAssetDaoDeps {
            CapiAssetDaoDeps {
                // TODO: review: what exactly has to be done on Capi app updates / escrow migrations?
                escrow: *self.capi_escrow.account.address(),
                escrow_percentage: self.capi_escrow_percentage,
                app_id: self.capi_app_id,
                asset_id: self.capi_asset_id,
            }
        }
    }

    fn msig() -> Result<TestsMsig> {
        Ok(TestsMsig::new(vec![msig_acc1(), msig_acc2(), msig_acc3()])?)
    }

    /// Common tests initialization
    pub async fn test_dao_init() -> Result<TestDeps> {
        test_init()?;

        let algod = dependencies::algod_for_tests();
        let api = LocalApi {};
        let capi_owner = capi_owner();

        let chain_deps = setup_on_chain_deps(&algod, &api, &capi_owner).await?;

        test_dao_init_with_deps(algod, &chain_deps).await
    }

    /// Use this for test initialization with custom chain deps
    pub async fn test_dao_init_with_deps(
        algod: Algod,
        chain_deps: &OnChainDeps,
    ) -> Result<TestDeps> {
        let OnChainDeps {
            funds_asset_id,
            capi_flow_res,
        } = chain_deps;

        Ok(TestDeps {
            algod,
            indexer: dependencies::indexer_for_tests(),
            api: Box::new(LocalApi {}),
            creator: creator(),
            investor1: investor1(),
            investor2: investor2(),
            customer: customer(),
            msig: msig()?,
            specs: dao_specs(),
            funds_asset_id: funds_asset_id.clone(),
            // unwrap: we know the owner mnemonic is valid + this is just for tests
            capi_owner: Account::from_mnemonic(&capi_flow_res.owner_mnemonic).unwrap(),
            precision: TESTS_DEFAULT_PRECISION,

            capi_escrow: capi_flow_res.escrow.clone(),
            capi_escrow_percentage: capi_escrow_percentage(),
            capi_app_id: capi_flow_res.app_id,
            capi_asset_id: capi_flow_res.asset_id,
            capi_supply: capi_flow_res.supply,

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

    fn funds_asset_creator() -> Account {
        // address: NIKGABIQLRCPJYCNCFZWR7GUIC3NA66EBVR65JKHKLGLIYQ4KO3YYPV67Q
        Account::from_mnemonic("accident inherit artist kid such wheat sure then skirt horse afford penalty grant airport school aim hollow position ask churn extend soft mean absorb achieve").unwrap()
    }

    fn test_accounts_initial_funds() -> FundsAmount {
        FundsAmount::new(10_000_000_000)
    }

    pub async fn create_and_distribute_funds_asset(algod: &Algod) -> Result<FundsAssetId> {
        let params = algod.suggested_transaction_params().await?;

        let asset_creator = funds_asset_creator();
        let asset_id = create_funds_asset(algod, &params, &asset_creator).await?;

        optin_and_fund_accounts_with_asset(
            algod,
            &params,
            asset_id.0,
            test_accounts_initial_funds(),
            &asset_creator,
            &vec![
                creator(),
                investor1(),
                investor2(),
                customer(),
                msig_acc1(),
                msig_acc2(),
                msig_acc3(),
            ],
        )
        .await?;
        Ok(asset_id)
    }

    /// Creates the funds asset and capi-token related dependencies
    pub async fn setup_on_chain_deps(
        algod: &Algod,
        api: &dyn Api,
        capi_owner: &Account,
    ) -> Result<OnChainDeps> {
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

        let capi_flow_res =
            create_capi_asset_and_deps(algod, api, capi_owner, funds_asset_id).await?;
        log::info!("capi_deps: {capi_flow_res:?}, funds_asset_id: {funds_asset_id:?}");

        Ok(OnChainDeps {
            funds_asset_id,
            capi_flow_res,
        })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OnChainDeps {
        pub funds_asset_id: FundsAssetId,
        pub capi_flow_res: CapiAssetFlowRes,
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

    async fn create_capi_asset_and_deps(
        algod: &Algod,
        api: &dyn Api,
        capi_owner: &Account,
        funds_asset_id: FundsAssetId,
    ) -> Result<CapiAssetFlowRes> {
        let capi_supply = CapiAssetAmount::new(1_000_000_000);
        Ok(setup_capi_asset_flow(algod, api, &capi_owner, capi_supply, funds_asset_id).await?)
    }

    fn capi_escrow_percentage() -> SharesPercentage {
        // unwraps: hardcoded value, which we knows works + this is used only in tests
        Decimal::from_str("0.1").unwrap().try_into().unwrap()
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

        log::debug!("Opted in and funded (funds asset): {}", receiver.address());

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

    #[test]
    #[ignore]
    async fn do_optin_capi_escrow_to_capi_asset_on_testnet() -> Result<()> {
        test_init()?;
        optin_capi_escrow_to_capi_asset(&Network::Test).await?;
        Ok(())
    }

    /// If the escrow's code changes, we have to setup (fund and opt-in to assets) it again
    /// Normally only used for Testnet / MainNet - for private networks everything is just re-created
    async fn optin_capi_escrow_to_capi_asset(net: &Network) -> Result<()> {
        test_init()?;

        let algod = algod_for_net(net);
        let params = algod.suggested_transaction_params().await?;

        // This can be any account that has enough assets (funds asset). Normally the funder will be the capi owner.
        let funder = capi_owner();

        let capi_escrow_template = test_capi_programs()?.escrow;

        let escrow = setup_and_submit_capi_escrow(
            &algod,
            &params,
            &funder,
            FundsAssetId(75503403), // pre-existing asset id
            CapiAssetId(77428422),  // pre-existing asset id
            CapiAppId(75503537),    // pre-existing app id
            &capi_escrow_template,
        )
        .await?;

        log::debug!("Finished capi escrow setup: {escrow:?}");

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
        let api = LocalApi {};
        let capi_owner = capi_owner();

        let deps = setup_on_chain_deps(&algod, &api, &capi_owner).await?;
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
    ) -> Result<()> {
        let build_config_str = match build_config {
            WasmBuildConfig::Debug => "debug",
            WasmBuildConfig::Release => "release",
        };

        let wasm_repo_path = "../frontend/wasm";
        let wasm_scrits_path = format!("{wasm_repo_path}/scripts");

        let wasm_local_build_script_path = format!("{wasm_scrits_path}/build_local.sh");

        let mut vars = generate_env_vars_for_config(&network, &env);
        let deps_vars = generate_env_vars_for_deps(deps);
        vars.extend(deps_vars);
        let vars_str = vars
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");

        let build_command = format!("wasm-pack build --out-dir ../wasm-build --{build_config_str}");
        let complete_build_command = format!("{vars_str} {build_command}");

        // Executes a WASM test that compiles the PyTeal (from the TEAL repo) and copies the output TEAL into the WASM project,
        // to ensure that the build's TEAL is up to date.
        let update_teal_command = "cargo test --package wasm --lib -- teal::update_teal::test::update_teal --exact --nocapture";

        let complete_command = format!("{update_teal_command}\n{complete_build_command}");

        write_to_file(wasm_local_build_script_path, &complete_command)?;

        Ok(())
    }

    fn generate_env_vars_for_config(network: &Network, env: &Env) -> Vec<(String, String)> {
        let network_str = match network {
            Network::SandboxPrivate => "sandbox_private",
            Network::Test => "test",
            Network::Private => "private",
        };
        let env_str = match env {
            Env::Test => "test",
            Env::Local => "local",
        };
        vec![
            ("NETWORK".to_owned(), network_str.to_owned()),
            ("ENV".to_owned(), env_str.to_owned()),
        ]
    }

    fn generate_env_vars_for_deps(deps: &OnChainDeps) -> Vec<(String, String)> {
        vec![
            (
                "FUNDS_ASSET_ID".to_owned(),
                deps.funds_asset_id.0.to_string(),
            ),
            (
                "CAPI_ESCROW_ADDRESS".to_owned(),
                deps.capi_flow_res.escrow.address().to_string(),
            ),
            (
                "CAPI_APP_ID".to_owned(),
                deps.capi_flow_res.app_id.0.to_string(),
            ),
            (
                "CAPI_ASSET_ID".to_owned(),
                deps.capi_flow_res.asset_id.0.to_string(),
            ),
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
    async fn hex_to_address() {
        let hex = "8c025cac37c404934e0066f5407032a6e2294b54026ee39fcd272b23643f5916";
        let bytes = HEXLOWER.decode(hex.as_bytes()).unwrap();
        let address = Address(bytes.try_into().unwrap());
        println!("Hex: {} -> address: {}", hex, address);
    }
}
