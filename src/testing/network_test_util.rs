#[cfg(test)]
use {
    crate::dependencies::{network, Network},
    crate::logger::init_logger,
    anyhow::Result,
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
fn reset_network(net: &Network) -> Result<()> {
    let mut cmd = Command::new("sh");

    let cmd_with_net_args = match net {
        &Network::SandboxPrivate => cmd
            .current_dir("scripts/sandbox")
            .arg("./sandbox_reset_for_tests.sh"),
        Network::Private => cmd
            .current_dir("scripts/private_net")
            .arg("./private_net_reset_for_tests.sh"),
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
