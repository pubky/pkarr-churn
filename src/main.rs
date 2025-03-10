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
//! - `max_hours`: Maximum duration (in hours) for the churn monitoring phase (default: 10 hours). The experiment stops
//!   after this duration even if the `stop_fraction` threshold is not met.
//!

use clap::Parser;
use pkarr::{Client, Keypair, PublicKey, SignedPacket};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Number of records to publish
    #[arg(long, default_value_t = 500)]
    num_records: usize,

    /// Stop after this fraction of records cannot be resolved (0.0 < x <= 1.0)
    #[arg(long, default_value_t = 1.1)]
    stop_fraction: f64,

    /// TTL (in seconds) for the published records
    #[arg(long, default_value_t = 604800)]
    ttl_s: u32,

    /// Sleep duration (in millis) between resolves
    #[arg(long, default_value_t = 1000)]
    sleep_duration_ms: u64,

    /// Maximum duration (in hours) for the churn monitoring phase
    #[arg(long, default_value_t = 72)]
    max_hours: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let client = Client::builder()
        .cache_size(0)
        .maximum_ttl(0)
        .no_relays()
        .build()?;

    let start = Instant::now();
    let published_records = publish_records(&client, cli.num_records, cli.ttl_s).await;
    println!(
        "Published {} records in {:?}",
        published_records.len(),
        start.elapsed()
    );

    println!("Wait one minute before starting to resolve records");

    let max_duration = Duration::from_secs(cli.max_hours * 3600);
    run_churn_loop(
        client,
        published_records,
        cli.stop_fraction,
        cli.sleep_duration_ms,
        max_duration,
    )
    .await?;

    Ok(())
}

async fn publish_records(
    client: &Client,
    num_records: usize,
    ttl_s: u32,
) -> Vec<(PublicKey, Instant)> {
    let mut records = Vec::with_capacity(num_records);
    let mut total_publish_duration: u64 = 0;
    for i in 0..num_records {
        let keypair = Keypair::random();
        let packet = match SignedPacket::builder()
            .txt(
                "_experiment".try_into().unwrap(),
                "dht-test".try_into().unwrap(),
                ttl_s,
            )
            .sign(&keypair)
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to build packet: {e}");
                continue;
            }
        };

        let publish_start = Instant::now();
        if let Err(e) = client.publish(&packet, None).await {
            eprintln!("Failed to publish record: {e:?}");
            continue;
        }
        let elapsed = publish_start.elapsed();
        total_publish_duration += elapsed.as_micros() as u64;

        records.push((keypair.public_key(), Instant::now()));

        let avg_secs = (total_publish_duration as f64) / ((i + 1) as f64 * 1_000_000.0);
        println!(
            "Published {} records: avg time per record: {:.6} s",
            i + 1,
            avg_secs
        );
    }

    records
}

async fn run_churn_loop(
    client: Client,
    verified_records: Vec<(PublicKey, Instant)>,
    stop_fraction: f64,
    sleep_duration_ms: u64,
    max_duration: Duration,
) -> anyhow::Result<()> {
    let total_keys = verified_records.len();
    let mut potential_churn: HashMap<PublicKey, Instant> = HashMap::new();

    let churn_start = Instant::now();
    loop {
        println!(
            "\n--- Churn pass; {} keys are currently marked as unresolved ---",
            potential_churn.len()
        );

        for (pubkey, _publish_instant) in &verified_records {
            tokio::time::sleep(Duration::from_millis(sleep_duration_ms)).await;

            // Try to resolve the key.
            if client.resolve(pubkey).await.is_some() {
                // If it had been marked as unresolved before, clear the flag.
                if potential_churn.remove(pubkey).is_some() {
                    println!("Key {pubkey} recovered; clearing failure record.");
                } else {
                    println!("Key {pubkey} is resolvable.");
                }
            } else {
                // If this is the first time we see a failure, record the time.
                if !potential_churn.contains_key(pubkey) {
                    potential_churn.insert(pubkey.clone(), Instant::now());
                    println!("Key {pubkey} unresolved; marking first failure timestamp.");
                } else {
                    println!("Key {pubkey} remains unresolved.");
                }
            }
        }

        // Save the current churn data to CSV at the end of each loop iteration.
        {
            let file = File::create("churns_500_7.csv")?;
            let mut writer = BufWriter::new(file);
            writeln!(writer, "pubkey,time_s")?;
            for (pubkey, publish_instant) in &verified_records {
                if let Some(failure_instant) = potential_churn.get(pubkey) {
                    let churn_time = failure_instant.duration_since(*publish_instant).as_secs();
                    writeln!(writer, "{pubkey},{churn_time}")?;
                } else {
                    writeln!(writer, "{pubkey},0")?;
                }
            }
            writer.flush()?;
        }

        let churn_fraction = potential_churn.len() as f64 / total_keys as f64;
        println!("Current churn fraction: {:.2}%", churn_fraction * 100.0);

        // Stop if the unresolved fraction threshold is reached.
        if churn_fraction >= stop_fraction {
            println!(
                "Stop fraction reached ({}%). Ending churn monitoring.",
                churn_fraction * 100.0
            );
            break;
        }

        // Also stop if the maximum duration has been exceeded.
        if churn_start.elapsed() >= max_duration {
            println!(
                "Maximum duration of {} hours reached. Ending churn monitoring.",
                max_duration.as_secs() / 3600
            );
            break;
        }
    }
    Ok(())
}
