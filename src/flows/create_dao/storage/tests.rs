#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tokio::test;

    use crate::{
        flows::create_dao::storage::{load_dao::load_dao, save_dao::save_dao},
        hashable::Hashable,
        testing::{flow::create_dao_flow::create_dao_flow, network_test_util::test_dao_init},
    };

    #[test]
    // For now ignore, as it needs a long delay (> 1 min) to wait for indexing
    // TODO: can we trigger indexing manually?
    #[ignore]
    async fn saves_and_loads_dao() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let create_dao_res = create_dao_flow(&td).await?;

        let to_sign = save_dao(&algod, &td.creator.address(), &create_dao_res.dao).await?;

        let signed_tx = td.creator.sign_transaction(to_sign.tx)?;

        let tx_id = algod.broadcast_signed_transaction(&signed_tx).await?.tx_id;

        println!(
            "Creator: {:?}, dao hash: {:?}, tx id: {:?}. Will wait for indexing..",
            td.creator.address(),
            to_sign.dao.hash()?,
            tx_id
        );

        std::thread::sleep(std::time::Duration::from_secs(70));

        println!("Fetching dao..");

        let dao_id = tx_id.parse()?;

        let stored_dao = load_dao(
            &algod,
            &td.indexer,
            &dao_id,
            &td.programs.escrows,
            &td.dao_deps(),
        )
        .await?;

        assert_eq!(create_dao_res.dao, stored_dao.dao);
        // double check
        assert_eq!(
            create_dao_res.dao.compute_hash()?,
            stored_dao.dao.compute_hash()?
        );

        Ok(())
    }
}
