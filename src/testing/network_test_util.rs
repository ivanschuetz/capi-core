#[cfg(test)]
pub use test::{test_dao_init, test_dao_with_funds_target_init, test_dao_with_specs, TestDeps};

#[cfg(test)]
mod test {
    use crate::algo_helpers::{send_tx_and_wait, send_txs_and_wait};
    use crate::dependencies::teal_api;
    use crate::flows::create_dao::setup_dao::Programs;
    use crate::teal::TealApi;
    use crate::testing::flow::create_dao_flow::test::test_programs;
    use algonaut::core::Address;
    use algonaut::{
        algod::v2::Algod,
        core::SuggestedTransactionParams,
        indexer::v2::Indexer,
        transaction::{account::Account, TransferAsset, TxnBuilder},
    };
    use chrono::{Duration, Utc};
    use data_encoding::{BASE64, HEXLOWER};
    use mbase::date_util::DateTimeExt;
    use mbase::dependencies::{algod, algod_for_net, algod_for_tests, indexer_for_tests, Network};
    use mbase::logger::init_logger;
    use mbase::models::asset_amount::AssetAmount;
    use mbase::models::capi_deps::{CapiAddress, CapiAssetDaoDeps};
    use mbase::models::dao_app_id::DaoAppId;
    use mbase::models::funds::{FundsAmount, FundsAssetId};
    use mbase::models::setup_dao_specs::SetupDaoSpecs;
    use mbase::models::shares_percentage::SharesPercentage;
    use mbase::state::dao_app_state::dao_global_state;
    use mbase::util::files::read_lines;
    use network_test_util::test_data::{
        capi_owner, creator, customer, dao_specs, dao_specs_with_funds_target, funds_asset_creator,
        investor1, investor2,
    };
    use network_test_util::tests_msig::TestsMsig;
    use network_test_util::{
        msig, optin_and_fund_accounts_with_asset, optin_and_send_asset_to_account,
        setup_on_chain_deps, test_init, OnChainDeps,
    };
    use rust_decimal::Decimal;
    use std::convert::TryInto;
    use std::str::FromStr;
    use tokio::test;
    use {crate::testing::TESTS_DEFAULT_PRECISION, anyhow::Result};

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

    /// inits logs, resets the network and initializes test dependencies for default test dao specs
    // TODO rename - it suggests that the dao is initialized here, which isn't the case - maybe crate_test_deps?
    /// Common tests initialization
    /// Guarantee: the returned funds raising end date is in the past and the target is 0,
    /// this means that the funds raising ended successfully,
    /// (which makes these deps backwards compatible with the pre-minfunds feature tests,
    /// where it's assumed that funds can always / unconditionally be withdrawn).
    pub async fn test_dao_init() -> Result<TestDeps> {
        test_dao_with_specs(&dao_specs()).await
    }

    /// inits logs, resets the network and initializes test dependencies
    /// Guarantee: the returned funds raising end date is in a week
    /// Relevant for test generally is that it's "later" by a safe span, so e.g. a withdrawal performed "now" with these deps will fail,
    /// as end date is in a week and withdrawals have to happen after it
    pub async fn test_dao_with_funds_target_init() -> Result<TestDeps> {
        // this needs to be dynamic, because we use a dynamic "now" reference date in TEAL and we've to ensure that this is after that
        // specifically 1 week doesn't have a particular reason, other than being a reasonable funding timeline generally
        let funds_end_date = Utc::now() + Duration::weeks(1);
        test_dao_with_specs(&dao_specs_with_funds_target(funds_end_date.to_timestap())).await
    }

    /// inits logs, resets the network and initializes test dependencies for given specs
    // named internal to not change the old "test_dao_init", which now means init without funds target specs
    pub async fn test_dao_with_specs(specs: &SetupDaoSpecs) -> Result<TestDeps> {
        test_init().await?;

        let algod = algod_for_tests();
        let capi_owner = capi_owner();

        let chain_deps = setup_on_chain_deps(&algod, &capi_owner).await?;

        test_dao_init_with_deps(algod, &chain_deps, specs).await
    }

    /// Use this for test initialization with custom chain deps
    async fn test_dao_init_with_deps(
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

    fn capi_escrow_percentage() -> SharesPercentage {
        // unwraps: hardcoded value, which we knows works + this is used only in tests
        Decimal::from_str("0.1").unwrap().try_into().unwrap()
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

    /// Reset and prepare local network for manual testing.
    #[test]
    #[ignore]
    async fn reset_and_fund_local_network() -> Result<()> {
        crate::testing::wasm::test::reset_and_fund_local_network().await?;
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
        // let deps = do_setup_on_chain_deps(&Network::Test).await?;
        // update_wasm_deps(&deps, WasmBuildConfig::Release, &Network::Test, &Env::Test)?;
        Ok(())
    }

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
            FundsAmount::new(100_000_000_000),
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

    /// Use this e.g. to fund DAO
    #[test]
    #[ignore]
    async fn send_payment() -> Result<()> {
        let receiver = "IW5LCUASFJCSJHEJEIT7SEQOBWZ52XHQD72UALYF5L2PG56KK636XPEG44"
            .parse()
            .unwrap();
        let funds_asset_id = FundsAssetId(12);

        let amount = FundsAmount::new(10_000_000_000);

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

    #[test]
    #[ignore]
    async fn print_dao_state() -> Result<()> {
        let app_id = DaoAppId(74);

        let algod = algod();
        let state = dao_global_state(&algod, app_id).await?;

        println!("State: {state:#?}");

        Ok(())
    }

    #[test]
    #[ignore]
    async fn base64_decode_uints64() -> Result<()> {
        let encoded = vec!["AAAAAGMQ6kk=", "AAAAAGMXKdY="];

        for e in encoded {
            println!(
                "{} => {}",
                e,
                // unwrap: this is a utility test
                u64::from_be_bytes(BASE64.decode(e.as_bytes()).unwrap().try_into().unwrap())
            );
        }
        Ok(())
    }
}
