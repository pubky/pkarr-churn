//! Publish and save the published public keys in a file
//! so they can be reused in other experiments.
//! 
//! Run with `cargo run --bin main_check_churn`.

use clap::Parser;
use helpers::count_dht_nodes_storing_packet;
use mainline::Dht;
use pkarr::{Keypair};
use published_key::PublishedKey;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    }, time::Duration,
};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

mod helpers;
mod published_key;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Number of records to publish
    #[arg(long, default_value_t = 50)]
    num_records: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("main_check_churn started.");
    // Initialize tracing

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()))
        .init();

    // Set up the Ctrl+C handler
    let ctrlc_pressed: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let r = ctrlc_pressed.clone();
    ctrlc::set_handler(move || {
        r.store(true, Ordering::SeqCst);
        println!("Ctrl+C detected, shutting down...");
    })
    .expect("Error setting Ctrl+C handler");

    println!("Press Ctrl+C to stop...");

    println!("Read published_secrets.txt");
    let published_keys = read_keys();

    println!("Read {} keys", published_keys.len());

    run_churn_loop(published_keys, &ctrlc_pressed).await;

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


async fn run_churn_loop(
    mut all_keys: Vec<PublishedKey>,
    ctrlc_pressed: &Arc<AtomicBool>,
) -> Vec<PublishedKey> {
    let client = Dht::client().unwrap();
    client.bootstrapped();
    let all_keys_count = all_keys.len();
    loop {
        let churn_count = all_keys.iter().filter(|key| key.is_churned()).count();
        let churn_fraction = churn_count as f64 / all_keys.len() as f64;
        info!("Current churn fraction: {:.2}%", churn_fraction * 100.0);

        for (i, key) in all_keys.iter_mut().enumerate() {
            let pubkey = &key.public_key();
            let nodes_count = count_dht_nodes_storing_packet(pubkey, &client);
            // Try to resolve the key.
            if nodes_count > 0 {
                info!("- {i}/{all_keys_count} Key {pubkey} is resolvable on {nodes_count} nodes.");
                key.mark_as_available();
            } else {
                info!("- {i}/{all_keys_count} Key {pubkey} unresolved");
                key.mark_as_churned();
            }

            if ctrlc_pressed.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        if ctrlc_pressed.load(Ordering::Relaxed) {
            break
        }

        info!("Sleep 1min before next loop");
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if ctrlc_pressed.load(Ordering::Relaxed) {
                break
            }
        }
        if ctrlc_pressed.load(Ordering::Relaxed) {
            break
        }
        println!("---------------------\n")
    }
    all_keys
}