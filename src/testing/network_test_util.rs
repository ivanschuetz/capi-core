#[cfg(test)]
use {
    crate::logger::init_logger,
    crate::dependencies::{network, Network},
    anyhow::Result,
    std::process::Command,
    std::{
        io::{BufRead, BufReader},
        process::Stdio,
    },
};

/// Common tests initialization
#[cfg(test)]
pub fn test_init() -> Result<()> {
    init_logger()?;
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
        // println!("{}", _line);
    }

    Ok(())
}
