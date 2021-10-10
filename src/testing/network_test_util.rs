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
        .current_dir("/Users/ischuetz/algo_nets")
        .arg("./reset_network.sh")
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .expect("Couldn't reset network");

    for line in BufReader::new(reset_res)
        .lines()
        .filter_map(|line| line.ok())
    {
        println!("{}", line);

        if line.starts_with("[online]") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let address = parts[1];
            call_fund_all(address)?;
        }
    }

    Ok(())
}

#[cfg(test)]
fn call_fund_all(address: &str) -> Result<()> {
    let res = Command::new("sh")
        .current_dir("/Users/ischuetz/algo_nets")
        .arg("./fund_all.sh")
        .arg(address.to_string())
        .arg("-d")
        .arg("~/algo_nets/net1/Node")
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .expect("Couldn't call fund_all.sh script");

    for line in BufReader::new(res).lines().filter_map(|line| line.ok()) {
        println!("{}", line);
    }

    Ok(())
}
