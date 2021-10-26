#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tokio::test;

    // use crate::{
    //     dependencies,
    //     flows::{
    //         create_project::setup::create_app::render_central_app,
    //         harvest::logic::{submit_harvest, HarvestSigned},
    //     },
    //     teal::load_teal_template,
    //     testing::TESTS_DEFAULT_PRECISION,
    // };

    // helper for environments that don't allow to open directly the TEAL debugger (e.g. WASM)
    // Copy the parameters, serialized to msg pack, here and run the test
    // (Note that Algonaut doesn't suppot JSON deserialization yet, otherwise we could use it alternatively)
    #[test]
    #[ignore]
    async fn debug_msg_pack_submit_par() -> Result<()> {
        // let algod = dependencies::algod();

        // // update rendered teal if needed - since teal was rendered with WASM,
        // // it's possible that the saved teal used here is outdated
        // let approval_template = load_teal_template("app_central_approval")?;
        // // insert current asset id and supply
        // let _ = render_central_app(approval_template, 6, 300, TESTS_DEFAULT_PRECISION)?;

        // // insert msg pack serialized bytes
        // let bytes = vec![];

        // // replace these with correct payload/submit call (if needed)
        // // it might be needed to temporarily derive Serialize/Deserialize
        // let signed: HarvestSigned = rmp_serde::from_slice(&bytes).unwrap();
        // submit_harvest(&algod, &signed).await?;

        Ok(())
    }
}
