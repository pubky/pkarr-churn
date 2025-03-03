use clap::Parser;
use mainline::Dht;
use pkarr::{Client, Keypair, PublicKey, SignedPacket};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    time::{Duration, Instant},
};
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Number of records to publish
    #[arg(long, default_value_t = 1024)]
    num_records: usize,

    /// Stop after this fraction of records cannot be resolved (0.0 < x <= 1.0)
    #[arg(long, default_value_t = 1.1)]
    stop_fraction: f64,

    /// TTL (in seconds) for the published records
    #[arg(long, default_value_t = 604800)]
    ttl_s: u32,

    /// Sleep duration (in milliseconds) between successive checks
    #[arg(long, default_value_t = 3000)]
    sleep_duration_ms: u64,

    /// Maximum duration (in hours) for the churn monitoring phase
    #[arg(long, default_value_t = 200)]
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

    // Obtain and bootstrap the DHT client.
    let dht = client.dht().unwrap();
    dht.clone().as_async().bootstrapped().await;

    // Publish records into the DHT.
    let published_records = publish_records(&client, cli.num_records, cli.ttl_s).await;
    println!("Published {} records.", published_records.len());

    println!("Waiting one minute before starting churn monitoring.");
    sleep(Duration::from_secs(60)).await;

    // Open CSV files.

    // 1. nodes_decay.csv: Logs changes for individual keys.
    let nodes_file = File::create("nodes_decay.csv")?;
    let mut nodes_writer = BufWriter::new(nodes_file);
    writeln!(nodes_writer, "timestamp_s,pubkey,nodes_count")?;

    // 2. churns.csv: Logs when a key goes unresolved (nodes count becomes 0).
    let churn_file = File::create("churns.csv")?;
    let mut churn_writer = BufWriter::new(churn_file);
    writeln!(churn_writer, "pubkey,churn_time_s")?;

    // 3. nodes_storing.csv: Logs the total global number of nodes across all keys whenever it decreases.
    let nodes_storing_file = File::create("nodes_storing.csv")?;
    let mut nodes_storing_writer = BufWriter::new(nodes_storing_file);
    writeln!(nodes_storing_writer, "node_count,timestamp")?;

    // Track the last known node count per key.
    let mut last_nodes_count: HashMap<PublicKey, u8> = HashMap::new();

    let max_duration = Duration::from_secs(cli.max_hours * 3600);
    run_churn_loop(
        dht,
        published_records,
        cli.stop_fraction,
        cli.sleep_duration_ms,
        max_duration,
        &mut nodes_writer,
        &mut last_nodes_count,
        &mut churn_writer,
        &mut nodes_storing_writer,
    )
    .await?;

    Ok(())
}

/// Publishes records into the DHT and returns a vector of (PublicKey, publication time).
async fn publish_records(
    client: &Client,
    num_records: usize,
    ttl_s: u32,
) -> Vec<(PublicKey, Instant)> {
    let mut records = Vec::with_capacity(num_records);

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

        if let Err(e) = client.publish(&packet, None).await {
            eprintln!("Failed to publish record: {e:?}");
            continue;
        }

        records.push((keypair.public_key(), Instant::now()));
        println!("Published record {}.", i + 1);
    }

    records
}

/// The churn loop monitors every published record and, for each one:
/// - Queries how many nodes (using `count_dht_nodes_storing_packet`) currently store its packet.
/// - Logs any change in the per-key node count to "nodes_decay.csv".
/// - Marks a key as churned (and logs it to "churns.csv") when its node count falls to 0.
/// - Additionally, calculates the global total number of nodes across all keys and,
///   whenever that total decreases, logs the new total and timestamp to "nodes_storing.csv".
async fn run_churn_loop(
    dht: Dht,
    verified_records: Vec<(PublicKey, Instant)>,
    stop_fraction: f64,
    sleep_duration_ms: u64,
    max_duration: Duration,
    nodes_writer: &mut BufWriter<File>,
    last_nodes_count: &mut HashMap<PublicKey, u8>,
    churn_writer: &mut BufWriter<File>,
    nodes_storing_writer: &mut BufWriter<File>,
) -> anyhow::Result<()> {
    let total_keys = verified_records.len();
    let mut potential_churn: HashMap<PublicKey, Instant> = HashMap::new();
    let churn_start = Instant::now();
    let mut last_global_count: Option<u32> = None;

    while churn_start.elapsed() < max_duration {
        println!(
            "Churn pass. Currently unresolved: {} keys",
            potential_churn.len()
        );

        // Process each key.
        for (pubkey, publish_instant) in &verified_records {
            sleep(Duration::from_millis(sleep_duration_ms)).await;

            // Query the current number of nodes storing the packet.
            let nodes_count = count_dht_nodes_storing_packet(pubkey, &dht).await;

            // If the per-key node count changed, log the update.
            let record_changed = match last_nodes_count.get(pubkey) {
                Some(&last) => last != nodes_count,
                None => true,
            };

            if record_changed {
                let timestamp = churn_start.elapsed().as_secs();
                writeln!(nodes_writer, "{timestamp},{pubkey},{nodes_count}")?;
                nodes_writer.flush()?;
                last_nodes_count.insert(pubkey.clone(), nodes_count);
            }

            // Check churn status: if no nodes hold the packet, mark it as churned.
            if nodes_count > 0 {
                if potential_churn.remove(pubkey).is_some() {
                    println!("Key {} recovered; clearing churn record.", pubkey);
                } else {
                    println!("Key {} is resolvable on {} nodes.", pubkey, nodes_count);
                }
            } else {
                if !potential_churn.contains_key(pubkey) {
                    potential_churn.insert(pubkey.clone(), Instant::now());
                    println!("Key {} unresolved; marking failure timestamp.", pubkey);
                    let churn_time = Instant::now().duration_since(*publish_instant).as_secs();
                    writeln!(churn_writer, "{pubkey},{churn_time}")?;
                    churn_writer.flush()?;
                } else {
                    println!("Key {} remains unresolved.", pubkey);
                }
            }
        }

        // Compute the global node count across all keys.
        let current_global_count: u32 = last_nodes_count.values().map(|&v| v as u32).sum();
        // If the global count decreased, log the new total.
        if let Some(prev) = last_global_count {
            if current_global_count < prev {
                let timestamp = churn_start.elapsed().as_secs();
                writeln!(nodes_storing_writer, "{current_global_count},{timestamp}")?;
                nodes_storing_writer.flush()?;
                println!(
                    "Global node count decreased from {} to {} at {} seconds.",
                    prev, current_global_count, timestamp
                );
            }
        }
        last_global_count = Some(current_global_count);

        let churn_fraction = potential_churn.len() as f64 / total_keys as f64;
        println!("Current churn fraction: {:.2}%", churn_fraction * 100.0);

        // Stop if the fraction of churned keys reaches the specified threshold.
        if churn_fraction >= stop_fraction {
            println!(
                "Stop fraction reached ({}%). Ending monitoring.",
                churn_fraction * 100.0
            );
            break;
        }
    }
    Ok(())
}

/// Asynchronous helper to count the number of DHT nodes storing a given packet.
/// This spawns a blocking task to iterate over the responses returned by `dht.get_mutable()`.
pub async fn count_dht_nodes_storing_packet(pubkey: &PublicKey, client: &Dht) -> u8 {
    let dht_clone = client.clone();
    let pubkey_clone = pubkey.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let stream = dht_clone.get_mutable(pubkey_clone.as_bytes(), None, None);
        let mut response_count: u8 = 0;
        for _ in stream {
            response_count += 1;
        }
        response_count
    });
    handle.await.unwrap()
}
