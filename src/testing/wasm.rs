#[cfg(test)]
pub mod test {
    /// NOTE: this should be in the WASM project - core shouldn't have any WASM dependencies. Temporary exception.
    /// WASM currently can't use tokio::test (for async tests)
    /// to fix this, we've to rename this project ("core" causes conflicts)
    ///
    use anyhow::Result;
    use mbase::{
        dependencies::{DataType, Env, Network},
        util::files::write_to_file,
    };
    use network_test_util::{do_setup_on_chain_deps, test_init, OnChainDeps};

    use crate::dependencies::TealApiLocation;

    // dead code: release config, usage commented
    #[allow(dead_code)]
    #[derive(Debug)]
    pub enum WasmBuildConfig {
        Debug,
        Release,
    }

    /// Sets up local environment for manual testing
    /// To be used by developers with have access to the wasm repo and dependencies
    pub async fn reset_and_fund_local_network() -> Result<()> {
        test_init().await?;
        let deps = do_setup_on_chain_deps(&Network::SandboxPrivate).await?;

        update_wasm_deps(
            &deps,
            WasmBuildConfig::Debug,
            &Network::SandboxPrivate,
            &Env::Local,
            &DataType::Real,
            TealApiLocation::Local,
        )?;

        Ok(())
    }

    /// Updates the WASM local build script with data (build-time environment variables)
    /// This way, after resetting the local network and re-creating basic data (e.g. funds asset id, which can change everytime)
    /// we can run this instead of having to update the build script manually
    fn update_wasm_deps(
        deps: &OnChainDeps,
        build_config: WasmBuildConfig,
        network: &Network,
        env: &Env,
        data_type: &DataType,
        teal_api_loc: TealApiLocation,
    ) -> Result<()> {
        let build_config_str = match build_config {
            WasmBuildConfig::Debug => "debug",
            WasmBuildConfig::Release => "release",
        };

        let wasm_repo_path = "../frontend/wasm";
        let wasm_scrits_path = format!("{wasm_repo_path}/scripts");

        let wasm_local_build_script_path = format!("{wasm_scrits_path}/build_local.sh");

        let mut vars = generate_env_vars_for_config(network, env, data_type, teal_api_loc);
        let deps_vars = generate_env_vars_for_deps(deps);
        vars.extend(deps_vars);
        let vars_str = vars
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");

        let build_command =
            format!("wasm-pack build --out-dir ../next/wasm --{build_config_str} --target web");
        let complete_build_command = format!("{vars_str} {build_command}");

        write_to_file(wasm_local_build_script_path, &complete_build_command)?;

        Ok(())
    }

    fn generate_env_vars_for_config(
        network: &Network,
        env: &Env,
        data_type: &DataType,
        teal_api_loc: TealApiLocation,
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
        let teal_api_loc_str = match teal_api_loc {
            TealApiLocation::Local => "local",
            TealApiLocation::Test => "test",
        };
        vec![
            ("NETWORK".to_owned(), network_str.to_owned()),
            ("ENV".to_owned(), env_str.to_owned()),
            ("DATA_TYPE".to_owned(), data_type_str.to_owned()),
            ("TEAL_API".to_owned(), teal_api_loc_str.to_owned()),
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
}
