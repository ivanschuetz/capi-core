use algonaut::algod::{v2::Algod, AlgodBuilder};

#[cfg(test)]
pub fn algod() -> Algod {
    private_network_algod()
}

#[allow(dead_code)]
fn private_network_algod() -> Algod {
    AlgodBuilder::new()
        .bind("http://127.0.0.1:4001")
        .auth("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .build_v2()
        .expect("Couldn't initialize algod")
}
