//! Publish and save the published public keys in a file
//! so they can be reused in other experiments.
//!
//! Run with `cargo run --bin main_publish_and_save`.

use clap::Parser;

use helpers::{count_dht_nodes_storing_packet, publish_records};
use pkarr::Client;
use published_key::PublishedKey;
use tokio::time::sleep;
use std::{
    process, sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    }, time::{Duration, Instant}
};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

mod helpers;
mod published_key;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Number of records to publish
    #[arg(long, default_value_t = 100)]
    num_records: usize,

    /// Number of parallel threads
    #[arg(long, default_value_t = 1)]
    threads: usize,
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
    let published_keys = publish_parallel(cli.num_records, cli.threads, &ctrlc_pressed).await;

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

async fn publish_parallel(num_records: usize, threads: usize, ctrlc_pressed: &Arc<AtomicBool>) -> Vec<PublishedKey> {
    let start = Instant::now();
    let client = Client::builder().no_relays().cache_size(0).build().unwrap();
    let mut handles = vec![];
    for thread_id in 0..threads {
        let c = client.clone();
        let handle = tokio::spawn(async move {
            tracing::info!("Started thread t{thread_id}");
            publish_records(num_records / threads, thread_id, c).await
        });
        handles.push(handle);
    }

    loop {
       let all_finished = handles.iter().map(|handle| handle.is_finished()).reduce(|a, b| a && b).unwrap();
       if all_finished {
            break
        }
        if ctrlc_pressed.load(Ordering::Relaxed) {
            break
        }
        sleep(Duration::from_millis(250)).await;
    }

    let mut all_result = vec![];
    for handle in handles {
        let keys = handle.await.unwrap();
        all_result.extend(keys);
    }

    tracing::info!("Published {} keys in {} seconds", all_result.len(), start.elapsed().as_secs());

    if ctrlc_pressed.load(Ordering::Relaxed) {
        process::exit(0);
    }

    all_result
}
