use algonaut::{algod::v2::Algod, indexer::v2::Indexer};

#[derive(Debug)]
pub enum Network {
    Private,
    SandboxPrivate,
    Test,
}

#[derive(Debug)]
pub enum Env {
    Local,
    Test,
}

pub fn network() -> Network {
    let str = option_env!("NETWORK");
    log::debug!("Network str: {:?}", str);

    let network = match str {
        Some("private") => Network::Private,
        Some("sandbox_private") => Network::SandboxPrivate,
        Some("test") => Network::Test,
        _ => {
            log::warn!("No network passed to build. Defaulting to SandboxPrivate.");
            Network::SandboxPrivate
        }
    };
    log::info!("Network: {:?}", network);
    network
}

pub fn env() -> Env {
    let str = option_env!("ENV");
    log::debug!("env str: {:?}", str);

    let env = match str {
        Some("test") => Env::Test,
        Some("local") => Env::Local,
        _ => {
            log::warn!("No environment passed to build. Defaulting to Local.");
            Env::Local
        }
    };
    log::info!("Env: {:?}", env);
    env
}

pub fn base_url<'a>() -> &'a str {
    match env() {
        Env::Local => "http://localhost:3000",
        Env::Test => "https://test.app.capi.finance",
    }
}

/// Convenience to not have to pass env everywhere
pub fn algod() -> Algod {
    algod_for_net(&network())
}

/// Convenience to not have to pass env everywhere
pub fn indexer() -> Indexer {
    indexer_for_net(&network())
}

pub fn algod_for_tests() -> Algod {
    // for tests there's no need to pass an environment - network is hardcoded
    algod_for_net(&Network::SandboxPrivate)
}

pub fn indexer_for_tests() -> Indexer {
    // for tests there's no need to pass an environment - network is hardcoded
    indexer_for_net(&Network::SandboxPrivate)
}

pub fn algod_for_net(network: &Network) -> Algod {
    match network {
        Network::SandboxPrivate => sandbox_private_network_algod(),
        Network::Private => private_network_algod(),
        Network::Test => testnet_algod(),
    }
}

fn indexer_for_net(network: &Network) -> Indexer {
    match network {
        Network::SandboxPrivate => sandbox_private_network_indexer(),
        Network::Private => {
            // Situational: since we've not needed indexer until now, the private network scripts don't support it.
            // and since we switched to sandbox, no reason to spend time with this currently.
            let msg = "Private network doesn't support indexer yet";
            log::error!("{}", msg); // for WASM, which doesn't see panic messages
            panic!("{}", msg);
        }
        Network::Test => testnet_indexer(),
    }
}

#[allow(dead_code)]
fn sandbox_private_network_algod() -> Algod {
    Algod::new(
        "http://127.0.0.1:4001",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .expect("Couldn't initialize sandbox algod")
}

#[allow(dead_code)]
fn private_network_algod() -> Algod {
    Algod::new(
        "http://127.0.0.1:53630",
        "44d70009a00561fe340b2584a9f2adc6fec6a16322554d44f56bef9e682844b9",
    )
    .expect("Couldn't initialize algod")
}

#[allow(dead_code)]
fn testnet_algod() -> Algod {
    Algod::with_headers("https://node.testnet.algoexplorerapi.io", vec![])
        .expect("Couldn't initialize algod")
}

#[allow(dead_code)]
fn sandbox_private_network_indexer() -> Indexer {
    Indexer::new("http://127.0.0.1:8980").expect("Couldn't initialize sandbox indexer")
}

#[allow(dead_code)]
fn testnet_indexer() -> Indexer {
    Indexer::new("https://algoindexer.testnet.algoexplorerapi.io")
        .expect("Couldn't initialize sandbox indexer")
}
