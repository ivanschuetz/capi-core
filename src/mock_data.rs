#[cfg(test)]
pub mod test {
    use std::{convert::TryInto, str::FromStr};

    use crate::{
        algo_helpers::send_txs_and_wait,
        flows::create_dao::{model::CreateSharesSpecs, setup_dao_specs::SetupDaoSpecs},
        testing::{
            create_and_submit_txs::pay_submit,
            flow::{
                create_dao_flow::create_dao_flow,
                customer_payment_and_drain_flow::customer_payment_and_drain_flow,
                invest_in_dao_flow::{invests_flow, invests_optins_flow},
            },
            network_test_util::{optin_and_fund_accounts_with_asset, test_dao_with_specs},
            test_data::funds_asset_creator,
        },
    };
    use algonaut::{
        algod::v2::Algod,
        core::{Address, MicroAlgos, SuggestedTransactionParams},
        transaction::{account::Account, TransferAsset, TxnBuilder},
    };
    use anyhow::Result;
    use chrono::{Duration, Utc};
    use mbase::models::{
        funds::{FundsAmount, FundsAssetId},
        hash::GlobalStateHash,
        share_amount::ShareAmount,
    };
    use rust_decimal::Decimal;
    use tokio::test;

    /// Resets the default test network (note: "test" here means the network configured for our tests - usually sandbox-private)
    /// and populates it with some data
    /// Intended use: demos, perhaps specific frontend QA.
    /// Note that for styling (frontend/css) specifically, we've a wasm build that provides mock data
    /// (which doesn't use a network at all - this way developers don't have to configure a sandbox / we don't have to deploy on testnet).
    #[test]
    #[ignore]
    pub async fn reset_and_init_with_mock_data() -> Result<()> {
        let td = &test_dao_with_specs(&dao_specs()).await?;
        let algod = &td.algod;

        let dao = create_dao_flow(td).await?;

        // accounts copied from test_accounts.txt (bottom to top, as top ones are using in test deps - just to ensure there are no clashes)
        let investors = vec![
            Account::from_mnemonic("belt cereal point clock camp unfold job similar february define empty throw long luggage item trap desert own grit upset security rough wear able lion").unwrap(),
            Account::from_mnemonic("oyster vendor broccoli wisdom plug visit viable captain social boil normal vocal insane canoe toy cube napkin stove spend park deny abstract chief absent water").unwrap(),
            Account::from_mnemonic("whisper bless barrel ketchup robot vapor message control almost language liquid knock ginger snow bullet canoe shiver logic believe bulb boost royal electric ability polar").unwrap(),
            Account::from_mnemonic("amount poet weasel rigid word lend proud ginger coral muffin umbrella choice document road grid awake mouse antique proud borrow future unaware weather about orbit").unwrap(),
            Account::from_mnemonic("bacon prepare reopen soft timber ball quiz goat patient antique practice adapt vanish allow add leg enroll skill citizen segment seek enroll main ability fancy").unwrap(),
            Account::from_mnemonic("loan list slender boil green price tiny position iron all correct bring fatal sausage oxygen need casual erode settle repeat october lounge helmet about pelican").unwrap(),
            Account::from_mnemonic("mass margin train spell since youth exercise heavy arm fluid slice fan gas maximum call bounce cotton scan poverty panic test depth priority ability false").unwrap(),
            Account::from_mnemonic("brown cinnamon business ginger face lumber term depend blade skull print crime guitar nose forest belt topic supply fix few luxury vendor prize about mutual").unwrap(),
            Account::from_mnemonic("soccer gold field cute sheriff thought flash ticket mercy skull spend action blouse city venture apology wink butter suspect wagon sea example obvious above ring").unwrap(),
            Account::from_mnemonic("soon cruel search pass summer toe sea ocean domain theme focus hungry large aunt collect merge proof happy twelve twenty priority suggest yellow above pyramid").unwrap(),
            Account::from_mnemonic("visa spawn cake cinnamon miracle manual squirrel lab hood leopard bike visa mechanic youth wine venture moment town elevator language horn depart later absorb swing").unwrap(),
            Account::from_mnemonic("ginger limb antique logic talent rose hurry truck moral occur leave essence balcony minimum endorse can cup apart catch ocean style kidney memory able hill").unwrap(),
            Account::from_mnemonic("proud inside repair local annual swear ocean foam humor blush drum used more stick wall length bench trend one indicate cradle exhaust purse above actor").unwrap(),
            Account::from_mnemonic("naive noble gather primary image dynamic antenna lunar assist grow trophy kangaroo dream left impose cycle south echo jealous hundred rhythm burst mandate abandon affair").unwrap(),
            Account::from_mnemonic("say belt coast utility robot ordinary camp dentist biology rack energy corn voice whisper patch banner echo raw nerve own emotion gallery offer absorb train").unwrap(),
            Account::from_mnemonic("glow wisdom spatial embody material radio guitar typical script monster horse atom fatigue cluster brand shadow artefact slight steak exchange repair butter retire abstract always").unwrap(),
            Account::from_mnemonic("own glass step concert foot used next rally rack meat table lounge wash drill asset load maze side hello cruise short brother fluid about giant").unwrap(),
            Account::from_mnemonic("happy hour caution village beauty tomorrow put dune noodle profit begin else hill gesture crowd mesh off guess yellow control local orbit idle ability copper").unwrap(),
            Account::from_mnemonic("rhythm sock scene profit iron large loan slow spray three stove property certain subway tongue fun blanket fury lawn summer swift rigid bag absorb ask").unwrap(),
            Account::from_mnemonic("industry rug level front public describe club crew physical horse setup genius random evil rapid silk confirm walk pear diet unfold wing slide ability alley").unwrap(),
            Account::from_mnemonic("hen betray innocent decorate dial volcano creek sentence embody fantasy chronic oppose later raccoon credit weapon soon zone hundred chef whip year width about fit").unwrap(),
            // adding this makes the holders pie chart end with the same color it started TODO fix (low prio)
            // Account::from_mnemonic("lion distance tone muffin tube prison organ reason museum fury radio system toy kid orange hero save future left sustain clay history net abandon slim").unwrap(),
        ];

        let funder = funds_asset_creator(); // account we know has enough algos and funds asset

        // let infos = algod.account_information(&funder.address()).await?;
        // dbg!(infos);

        let params = algod.suggested_transaction_params().await?;

        log::info!("will send algos to investors..");

        // send algos to investors (needed to pay fees)
        for investor in &investors {
            pay_submit(
                algod,
                &params,
                &funder,
                &investor.address(),
                MicroAlgos(100_000_000),
            )
            .await?;
        }

        log::info!("will send funds asset to investors..");

        // fund investors (fund asset - needed to buy shares)
        optin_and_fund_accounts_with_asset(
            algod,
            &params,
            dao.funds_asset_id.0,
            FundsAmount::new(100_000_000_000),
            &funder,
            &investors,
        )
        .await?;

        log::info!("starting investments and payments..");

        // the drain test flow uses the customer as payment sender, so fund it
        send_funds(
            algod,
            &params,
            FundsAmount::new(100_000_000_000),
            &funder,
            &td.customer.address(),
            dao.funds_asset_id,
        )
        .await?;

        // do some transactions
        // we do them in the same iteration to have more varied-looking funds activity
        // (instead of a block of investments, then a block of payments..)
        for investor in &investors {
            // invest
            invests_optins_flow(algod, &investor, &dao).await?;
            invests_flow(td, &investor, ShareAmount::new(200_000), &dao).await?;

            // send a payment and drain (needed for withdrawals)
            customer_payment_and_drain_flow(&td, &dao, FundsAmount::new(1_000_000_000), &funder)
                .await?;

            // // withdraw some funds
            // // commented: we can't withdraw while there's active raising phase
            // // for now we prioritize showing the later, as it makes for a better demo
            // withdraw_flow(
            //     algod,
            //     &dao,
            //     &td.creator,
            //     FundsAmount::new(200_000_000),
            //     dao.id().0,
            // )
            // .await?;
        }

        // show app id - used in url
        log::info!("finished initializing mock data. Dao id: {:?}", dao.id());

        Ok(())
    }

    pub fn dao_specs() -> SetupDaoSpecs {
        // unwrap: tests, and we know hardcoded data is correct
        SetupDaoSpecs::new(
            "Hello World Ltd".to_owned(),
            // Some(GlobalStateHash("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore".to_owned())),
            None,
            CreateSharesSpecs {
                token_name: "COOL".to_owned(),
                supply: ShareAmount::new(10_000_000),
            },
            Decimal::from_str("0.4").unwrap().try_into().unwrap(),
            FundsAmount::new(100_000),
            Some(GlobalStateHash("test_hash".to_owned())),
            Some("123".to_owned()),
            "https://twitter.com/capi_fin".to_owned(),
            ShareAmount::new(10_000_000),
            FundsAmount::new(200_000_000_000),
            (Utc::now() + Duration::weeks(4)).into(),
        )
        .unwrap()
    }

    async fn send_funds(
        algod: &Algod,
        params: &SuggestedTransactionParams,
        amount: FundsAmount,
        sender: &Account,
        receiver: &Address,
        asset_id: FundsAssetId,
    ) -> Result<()> {
        let tx = TxnBuilder::with(
            &params,
            TransferAsset::new(sender.address(), asset_id.0, amount.val(), *receiver).build(),
        )
        .build()?;
        let fund_tx_signed = sender.sign_transaction(tx)?;
        send_txs_and_wait(&algod, &[fund_tx_signed]).await?;

        println!("Funded!");

        Ok(())
    }
}
