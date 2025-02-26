//! # DHT Churn Experiment
//!
//! This example demonstrates how to measure the distribution of churn times of Pkarr records from Mainline DHT.
//!
//! Pkarr records may become non-resolvable (i.e. churned), most likely due to network node churn or due to eviction
//! from nodes due to cache being full.
//!
//! ## Why Run This Experiment?
//!
//! - **Network Dynamics Analysis**: Understand how long records persist in the DHT under realistic network conditions.
//! - **Research and Diagnostics**: Provide insights for researchers and developers into the operational characteristics
//!   of distributed systems.
//! - **Performance Tuning**: Adjust republishing strategies and related parameters based on observed churn behavior.
//!
//! ## How It Works
//!
//! 1. **Publishing Phase**: A specified number of records (defaults to 500) are published sequentially into the DHT with a given TTL.
//!    The publishing progress is logged along with the average time per publish.
//! 2. **Churn Phase**: In a loop, the experiment periodically attempts to resolve the published records.
//!    The experiment stops when either:
//!    - A preconfigured fraction of the records have churned (defaults to 0.8), or
//!    - A specified maximum observation duration (defaults to 12 hours) has elapsed.
//!
//!    When a record is no longer resolvable, its churn time (i.e. the elapsed time since publication) is recorded
//!    in a CSV file. Remaining active records at the end of the experiment are logged with a churn time of 0.
//!
//! ## Limitations
//!
//! - **Network Variability**: The measured churn times may be influenced by transient network latency and load.
//! - **Time Granularity**: The sleep duration between resolution passes limits the precision of the churn time measurements.
//! - **Incomplete Churns**: Some records may not churn during the observation period, potentially skewing the data.
//! - **Assumption on Churn**: We assume a record has churned the first time `pkarr.resolve()` returns `None`.
//!   This might happen for different reasons and does not necessarily mean the record was permanently lost.
//!
//! ## Configuration
//!
//! The experiment can be configured via command-line arguments:
//!
//! - `num_records`: Total number of records to publish (default: 500).
//! - `stop_fraction`: The fraction of records that, once churned, will stop the experiment (default: 0.8).
//! - `ttl_s`: Time-to-live (in seconds) for each published record (default: 1 week).
//! - `sleep_duration_ms`: Duration (in milliseconds) to wait between successive resolves (default: 1000 ms).
//! - `max_hours`: Maximum duration (in hours) for the churn monitoring phase (default: 12 hours). The experiment stops
//!   after this duration even if the `stop_fraction` threshold is not met.
//!

use clap::Parser;
use helpers::count_dht_nodes_storing_packet;
use mainline::Dht;
use pkarr::Client;
use published_key::PublishedKey;
use std::{
    fs::File,
    io::{BufWriter, Write},
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
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

    /// Maximum duration (in hours) for the churn monitoring phase
    #[arg(long, default_value_t = 4*24)]
    max_hours: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("pkarr_churn_experiment started.");
    // Initialize tracing
    let filter = EnvFilter::from_default_env()
        .add_directive(LevelFilter::ERROR.into())
        .add_directive("pkarr_churn_experiment=trace".parse().unwrap());
    tracing_subscriber::fmt().with_env_filter(filter).init();

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

    let start = Instant::now();

    info!("Publish {} records", cli.num_records);
    let published_keys = publish_records(cli.num_records, &ctrlc_pressed).await;
    info!(
        "Published {} records in {:?}",
        published_keys.len(),
        start.elapsed()
    );
    println!();

    run_churn_loop(published_keys, &ctrlc_pressed).await;

    Ok(())
}

/// Publish x packets
async fn publish_records(num_records: usize, ctrlc_pressed: &Arc<AtomicBool>) -> Vec<PublishedKey> {
    let client = Client::builder().no_relays().build().unwrap();
    let mut records = Vec::with_capacity(num_records);

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
            exit(0);
        }
    }
    records
}

async fn run_churn_loop(
    mut all_keys: Vec<PublishedKey>,
    ctrlc_pressed: &Arc<AtomicBool>,
) -> Vec<PublishedKey> {
    let file = File::create("churns.csv").unwrap();
    let mut writer = BufWriter::new(file);
    writeln!(writer, "pubkey,time_s").unwrap();

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
        println!("---------------------\n")
    }
    all_keys
}

// // Final logging: for keys that remain unresolved, log the churn time
// // (difference between the first failure and publication). Keys that never failed
// // are logged with a churn time of 0.
// for (pubkey, publish_instant) in &all_keys {
//     if let Some(failure_instant) = potential_churn.get(pubkey) {
//         let churn_time = failure_instant.duration_since(*publish_instant).as_secs();
//         writeln!(writer, "{pubkey},{churn_time}")?;
//     } else {
//         writeln!(writer, "{pubkey},0")?;
//     }
// }
// writer.flush()?;
// Ok(())

//     if let Some(failure_instant) = potential_churn.get(pubkey) {
//         let churn_time = failure_instant.duration_since(*publish_instant).as_secs();
//         writeln!(writer, "{pubkey},{churn_time}")?;
//     } else {
//         writeln!(writer, "{pubkey},0")?;
//     }
// }
// writer.flush()?;
// Ok(())
