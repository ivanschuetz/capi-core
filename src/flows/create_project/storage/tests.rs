#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tokio::test;

    use crate::{
        flows::create_project::storage::{load_project::load_project, save_project::save_project},
        hashable::Hashable,
        testing::{
            flow::create_project_flow::create_project_flow, network_test_util::test_dao_init,
        },
    };

    #[test]
    // For now ignore, as it needs a long delay (> 1 min) to wait for indexing
    // TODO: can we trigger indexing manually?
    #[ignore]
    async fn saves_and_loads_project() -> Result<()> {
        let td = test_dao_init().await?;
        let algod = &td.algod;

        let create_project_res = create_project_flow(&td).await?;

        let to_sign =
            save_project(&algod, &td.creator.address(), &create_project_res.project).await?;

        let signed_tx = td.creator.sign_transaction(&to_sign.tx)?;

        let tx_id = algod.broadcast_signed_transaction(&signed_tx).await?.tx_id;

        println!(
            "Creator: {:?}, project hash: {:?}, tx id: {:?}. Will wait for indexing..",
            td.creator.address(),
            to_sign.project.hash()?,
            tx_id
        );

        std::thread::sleep(std::time::Duration::from_secs(70));

        println!("Fetching project..");

        let project_id = tx_id.parse()?;

        let stored_project = load_project(
            &algod,
            &td.indexer,
            &project_id,
            &td.programs.escrows,
            &td.dao_deps(),
        )
        .await?;

        assert_eq!(create_project_res.project, stored_project.project);
        // double check
        assert_eq!(
            create_project_res.project.compute_hash()?,
            stored_project.project.compute_hash()?
        );

        Ok(())
    }
}
