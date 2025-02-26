//! Publish and save the published public keys in a file
//! so they can be reused in other experiments.
//! 
//! Run with `cargo run --bin main_publish_and_save`.

use clap::Parser;

use pkarr::Client;
use published_key::PublishedKey;
use std::{
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
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
    println!("main_publish_and_save started.");
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

    let cli = Cli::parse();

    info!("Publish {} records", cli.num_records);
    let published_keys = publish_records(cli.num_records, &ctrlc_pressed).await;

    // Turn into a hex list and write to file
    let pubkeys = published_keys.into_iter().map(|key| {
        let secret = key.key.secret_key();
        let h = hex::encode(secret);
        h
    }).collect::<Vec<_>>();
    let pubkeys_str = pubkeys.join("\n");
    std::fs::write("published_secrets.txt", pubkeys_str).unwrap();
    println!("Successfully wrote secrets keys to published_secrets.txt");
    Ok(())
}

/// Publish x packets
async fn publish_records(num_records: usize, ctrlc_pressed: &Arc<AtomicBool>) -> Vec<PublishedKey> {
    let client = Client::builder().no_relays().build().unwrap();
    let mut records = vec![];

    for i in 0..num_records {
        let key = PublishedKey::random();
        let packet = key.create_packet();
        if let Err(e) = client.publish(&packet, None).await {
            error!("Failed to publish {} record: {e:?}", key.public_key());
            continue;
        }
        info!("- {i}/{num_records} Published {}", key.public_key());
        records.push(key);

        if ctrlc_pressed.load(Ordering::Relaxed) {
            break
        }
    }
    records
}
