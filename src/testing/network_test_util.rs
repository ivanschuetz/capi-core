#[cfg(test)]
use {
    crate::logger::init_logger,
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
    reset_network()?;
    Ok(())
}

#[cfg(test)]
fn reset_network() -> Result<()> {
    let reset_res = Command::new("sh")
        .current_dir("scripts/sandbox")
        .arg("./sandbox_reset_for_tests.sh")
        // .current_dir("scripts/private_net")
        // .arg("./private_net_reset_for_tests.sh")
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
