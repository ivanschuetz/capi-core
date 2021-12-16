use std::fs;

use algonaut::transaction::SignedTransaction;
use anyhow::{anyhow, Result};
use serde::Serialize;
use tealdbg::Config;
// use tealdbg::Config;
use tinytemplate::TinyTemplate;

// not rendered teal template (with placeholders)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TealSourceTemplate(pub Vec<u8>);

// regular teal source (not a template)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TealSource(pub Vec<u8>);

impl ToString for TealSource {
    fn to_string(&self) -> String {
        // unwrap: for now we'll assume that this struct is always initialized with valid utf-8,
        // TODO (low prio) actually ensure it
        String::from_utf8(self.0.clone()).unwrap()
    }
}

/// file_name without .teal
/// use this to debug with debug_teal_rendered
pub fn save_rendered_teal(file_name: &str, teal: TealSource) -> Result<()> {
    let folder = "teal_rendered";
    Ok(fs::write(format!("{}/{}.teal", folder, file_name), teal.0)?)
}

// file_name without .teal
pub fn load_teal_template(file_name: &str) -> Result<TealSourceTemplate> {
    load_file_bytes("teal_template", file_name).map(TealSourceTemplate)
}

// file_name without .teal
pub fn load_teal(file_name: &str) -> Result<TealSource> {
    load_file_bytes("teal", file_name).map(TealSource)
}

fn load_file_bytes(folder: &str, file_name: &str) -> Result<Vec<u8>> {
    Ok(fs::read(format!("{}/{}.teal", folder, file_name))?)
}

pub fn render_template<T>(template: TealSourceTemplate, context: T) -> Result<TealSource>
where
    T: Serialize,
{
    let mut tt = TinyTemplate::new();
    let teal_str = &String::from_utf8(template.0)?;
    let template_identifier = "program"; // arbitrary identifier, see tinytemplate docs
    tt.add_template(template_identifier, teal_str)?;

    let rendered = tt.render(template_identifier, &context)?;
    Ok(TealSource(rendered.as_bytes().to_vec()))
}

/// file_name without .teal
#[allow(dead_code)]
pub fn debug_teal(txs: &[SignedTransaction], file_name: &str) -> Result<()> {
    debug_teal_internal(txs, "teal", file_name)
}

/// file_name without .teal
/// separate folder for rendered templates to easily add to .gitignore
#[allow(dead_code)]
pub fn debug_teal_rendered(txs: &[SignedTransaction], file_name: &str) -> Result<()> {
    debug_teal_internal(txs, "teal_rendered", file_name)
}

/// file_name without .teal
#[allow(dead_code)]
fn debug_teal_internal(txs: &[SignedTransaction], folder: &str, file_name: &str) -> Result<()> {
    tealdbg::launch(
        Config {
            // node_dir: Some("/Users/ischuetz/algo_nets/net1/Node"),
            ..Config::default()
        },
        txs,
        format!("{}/{}.teal", folder, file_name),
    )
    .map_err(|e| anyhow!(e))
}
