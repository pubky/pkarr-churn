//! # Maximum Publish Speed Experiment
//!
//! This example continuously publishes records to the DHT to measure the maximum publish speed.
//! It uses multiple Tokio threads (64 by default), each with its own client.
//!
//! ## Key Features
//!
//! - **Concurrency**: Uses 64 concurrent Tokio tasks by default.
//! - **Client per Thread**: Each thread creates a new client instance.
//! - **Metrics Tracking**: Keeps track of success and failure counts for publishing.
//! - **Reporting**: Every _N_ successful publishes (configurable) it prints:
//!   - Total attempts and success/failure ratio.
//!   - Average time per successful publish.
//!   - Estimated number of keys published in an hour.
//!
//! ## Configuration Options
//!
//! - `ttl_s`: TTL (in seconds) for each published record (default: 604800 seconds or 1 week).
//! - `report_interval`: Print statistics every N successful publishes (default: 1000).
//! - `threads`: Number of Tokio tasks (threads) to spawn (default: 64).
//!

use clap::Parser;
use pkarr::{Client, Keypair, SignedPacket};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// TTL (in seconds) for the published records (default: 604800 seconds, 1 week)
    #[arg(long, default_value_t = 604800)]
    ttl_s: u32,

    /// Print statistics every N successful publishes
    #[arg(long, default_value_t = 50)]
    report_interval: usize,

    /// Number of Tokio threads to use
    #[arg(long, default_value_t = 128)]
    threads: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Global atomic counters for successes and failures.
    let success_count = Arc::new(AtomicUsize::new(0));
    let failure_count = Arc::new(AtomicUsize::new(0));
    let start_time = Instant::now();

    // Spawn the specified number of concurrent tasks.
    let mut handles = Vec::with_capacity(cli.threads);
    for _ in 0..cli.threads {
        let success_count = Arc::clone(&success_count);
        let failure_count = Arc::clone(&failure_count);
        let ttl_s = cli.ttl_s;
        let report_interval = cli.report_interval;
        let start_time = start_time.clone();

        let handle = tokio::spawn(async move {
            // Create a new client for this thread.
            let client = Client::builder().build().expect("failed to create client");

            loop {
                // Create a new record.
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
                        failure_count.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                // Publish the record.
                match client.publish(&packet, None).await {
                    Ok(_) => {
                        // Increment the success counter.
                        let successes = success_count.fetch_add(1, Ordering::Relaxed) + 1;
                        // Report statistics every N successful publishes.
                        if successes % report_interval == 0 {
                            let failures = failure_count.load(Ordering::Relaxed);
                            let total_attempts = successes + failures;
                            let elapsed = start_time.elapsed().as_secs_f64();
                            let avg_publish_time = elapsed / (successes as f64);
                            let estimated_per_hour = (successes as f64 / elapsed) * 3600.0;
                            println!(
                                "Total attempts: {} | Success: {} | Failures: {} | Success Ratio: {:.2}%",
                                total_attempts,
                                successes,
                                failures,
                                (successes as f64 / total_attempts as f64) * 100.0,
                            );
                            println!(
                                "Avg time per successful publish: {:.6} s | Estimated publishes per hour: {:.0}",
                                avg_publish_time, estimated_per_hour
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to publish record: {e:?}");
                        failure_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to finish (this example runs indefinitely).
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}
