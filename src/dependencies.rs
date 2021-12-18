use algonaut::algod::{v2::Algod, AlgodBuilder};

#[cfg(test)]
pub fn algod() -> Algod {
    private_network_algod()
}

#[allow(dead_code)]
fn private_network_algod() -> Algod {
    AlgodBuilder::new()
        // .bind("http://127.0.0.1:4001") // sandbox
        // .auth("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa") // sandbox
        .bind("http://127.0.0.1:53630")
        .auth("44d70009a00561fe340b2584a9f2adc6fec6a16322554d44f56bef9e682844b9")
        .build_v2()
        .expect("Couldn't initialize algod")
}
