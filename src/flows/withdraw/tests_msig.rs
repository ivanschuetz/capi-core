// commented: there's no separate owner anymore
// msig just means that creator was rekeyed to msig account
// we probably should test msig anyway, after rekey (low prio)
// since rekeying is not fully functional yet, this is frozen
//
// #[cfg(test)]
// mod tests {
//     use crate::{
//         state::account_state::funds_holdings,
//         testing::{
//             flow::{
//                 create_dao_flow::create_dao_flow,
//                 withdraw_flow::{
//                     test::{withdraw_incomplete_msig_flow, withdraw_msig_flow},
//                     withdraw_precs,
//                 },
//             },
//             network_test_util::test_dao_init,
//         },
//     };
//     use anyhow::Result;
//     use mbase::{
//         checked::{CheckedAdd, CheckedSub},
//         models::funds::FundsAmount,
//     };
//     use serial_test::serial;
//     use tokio::test;

//     #[test]
//     #[serial]
//     async fn test_withdraw_msig_success() -> Result<()> {
//         let td = &test_dao_init().await?;
//         let algod = &td.algod;
//         let drainer = &td.investor1;

//         // precs

//         let withdraw_amount = FundsAmount::new(1_000_000);

//         let dao = create_dao_flow(&td).await?;
//         let pay_and_drain_amount = FundsAmount::new(10 * 1_000_000);

//         withdraw_precs(td, drainer, &dao, pay_and_drain_amount).await?;

//         // remeber state
//         let msig_balance_bafore_withdrawing =
//             funds_holdings(&algod, &td.msig.address().address(), td.funds_asset_id).await?;
//         let app_balance_before_withdrawing =
//             funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
//         let creator_balance_bafore_withdrawing =
//             funds_holdings(&algod, &td.creator.address(), td.funds_asset_id).await?;

//         // flow

//         withdraw_msig_flow(
//             &algod,
//             &td.msig,
//             withdraw_amount,
//             dao.app_id,
//             dao.funds_asset_id,
//         )
//         .await?;

//         // test

//         // msig got the funds
//         let msig_funds =
//             funds_holdings(algod, &td.msig.address().address(), td.funds_asset_id).await?;
//         assert_eq!(
//             msig_balance_bafore_withdrawing
//                 .add(&withdraw_amount)
//                 .unwrap(),
//             msig_funds
//         );

//         // central lost the funds
//         let app_amount = funds_holdings(algod, &dao.app_address(), td.funds_asset_id).await?;
//         assert_eq!(
//             app_balance_before_withdrawing
//                 .sub(&withdraw_amount)
//                 .unwrap(),
//             app_amount
//         );

//         // sanity check: the creator's balance didn't change
//         let creator_funds = funds_holdings(algod, &td.creator.address(), td.funds_asset_id).await?;
//         assert_eq!(creator_balance_bafore_withdrawing, creator_funds);

//         Ok(())
//     }

//     /// This is testing more Algorand/the SDK than Capi, but why not. Might delete.
//     #[test]
//     #[serial]
//     async fn test_withdraw_incomplete_msig_fails() -> Result<()> {
//         let td = &test_dao_init().await?;
//         let algod = &td.algod;
//         let drainer = &td.investor1;

//         // precs

//         let withdraw_amount = FundsAmount::new(1_000_000);

//         let dao = create_dao_flow(&td).await?;
//         let pay_and_drain_amount = FundsAmount::new(10 * 1_000_000);

//         withdraw_precs(td, drainer, &dao, pay_and_drain_amount).await?;

//         // remeber state
//         let msig_balance_bafore_withdrawing =
//             funds_holdings(&algod, &td.msig.address().address(), td.funds_asset_id).await?;
//         let app_balance_before_withdrawing =
//             funds_holdings(&algod, &dao.app_address(), td.funds_asset_id).await?;
//         let creator_balance_bafore_withdrawing =
//             funds_holdings(&algod, &td.creator.address(), td.funds_asset_id).await?;

//         // flow

//         let res = withdraw_incomplete_msig_flow(
//             &algod,
//             &td.msig,
//             withdraw_amount,
//             dao.app_id,
//             dao.funds_asset_id,
//         )
//         .await;

//         // test

//         log::debug!("Withdraw res: {res:?}");
//         assert!(res.is_err());

//         // msig funds didn't change
//         let msig_funds =
//             funds_holdings(algod, &td.msig.address().address(), td.funds_asset_id).await?;
//         assert_eq!(msig_balance_bafore_withdrawing, msig_funds);

//         // central funds didn't change
//         let app_amount = funds_holdings(algod, &dao.app_address(), td.funds_asset_id).await?;
//         assert_eq!(app_balance_before_withdrawing, app_amount);

//         // sanity check: the creator's balance didn't change
//         let creator_funds = funds_holdings(algod, &td.creator.address(), td.funds_asset_id).await?;
//         assert_eq!(creator_balance_bafore_withdrawing, creator_funds);

//         Ok(())
//     }
// }
