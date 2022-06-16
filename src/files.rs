use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::Path;

#[allow(dead_code)]
pub fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

#[allow(dead_code)]
pub fn write_to_file<P>(path: P, str: &str) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)
        .unwrap();

    file.write_all(str.as_bytes())?;

    Ok(())
}
