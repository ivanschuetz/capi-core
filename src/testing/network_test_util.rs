#[cfg(test)]
use {
    anyhow::Result,
    std::process::Command,
    std::{
        io::{BufRead, BufReader},
        process::Stdio,
    },
};

#[cfg(test)]
pub fn reset_network() -> Result<()> {
    let reset_res = Command::new("sh")
        .current_dir("scripts")
        .arg("./sandbox_reset_for_tests.sh")
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .expect("Couldn't reset network");

    for _ in BufReader::new(reset_res)
        .lines()
        .filter_map(|line| line.ok())
    {
        // println!("{}", line);
    }

    Ok(())
}
