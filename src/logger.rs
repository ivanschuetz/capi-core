#[cfg(not(target_arch = "wasm32"))]
use anyhow::Result;

#[allow(dead_code)]
pub fn init_logger() -> Result<()> {
    match log4rs::init_file("./log_config.yml", Default::default()) {
        Ok(()) => Ok(()),
        Err(error) => {
            if error.to_string().contains(
                "attempted to set a logger after the logging system was already initialized",
            ) {
                // we don't have a common entry point for tests or the app, so it's valid to call init_logger repeatedly
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}
