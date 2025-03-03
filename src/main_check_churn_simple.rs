//! Publish and save the published public keys in a file
//! so they can be reused in other experiments.
//!
//! Run with `cargo run --bin main_check_churn`.

use helpers::count_dht_nodes_storing_packet;
use mainline::Dht;
use pkarr::{Keypair, PublicKey};
use published_key::PublishedKey;

mod helpers;
mod published_key;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("main_check_churn_simple started.");
    let mut tracing_initialized = false;

    let dht = Dht::client().unwrap();
    let keys = read_keys();
    let key_count = keys.len();
    for (i, key) in keys.iter().enumerate() {
        let count = count_dht_nodes_storing_packet(&key.public_key(), &dht);
        println!("- {i}/{key_count} {} Count {count}", key.public_key());
        if tracing_initialized {
            break;
        }
        if  !tracing_initialized && count == 0 {
            println!("Init tracing");
            tracing_subscriber::fmt::init();
            tracing_initialized = true;
        }
    }

    Ok(())
}


/// reads keys from published_secrets.txt
fn read_keys() -> Vec<PublishedKey> {
    let secret_srs = std::fs::read_to_string("published_secrets.txt").expect("File not found");
    let keys = secret_srs.lines().map(|line| line.to_string()).collect::<Vec<_>>();
    keys.into_iter().map(|key| {
        let secret = hex::decode(key).expect("invalid hex");
        let secret: [u8; 32] = secret.try_into().unwrap();
        let key = Keypair::from_secret_key(&secret);
        PublishedKey::new(key)
    }).collect::<Vec<_>>()
}

