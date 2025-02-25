//! MRE to replicate the resolve bug.
//! This script publishes 100 pkarr packets and tries to resolve them after.
//! `resolve_most_recent()` can't find them. Only when a new pkarr client is created without any caching limitations, it is able to resolve.
//! `resolve()` itself is working better but also returning missing packets occasionally even though they should be there.
use std::time::Duration;

use clap::Parser;
use pkarr::{Client, Keypair, PublicKey, SignedPacket};


#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Number of records to publish
    #[arg(long, default_value_t = 100)]
    num_records: usize,

    /// Stop after this fraction of records cannot be resolved (0.0 < x <= 1.0)
    #[arg(long, default_value_t = 0.90)]
    stop_fraction: f64,

    /// TTL (in seconds) for the published records
    #[arg(long, default_value_t = 604800)]
    ttl_s: u32,

    /// Sleep duration (in millis) between resolves
    #[arg(long, default_value_t = 1000)]
    sleep_duration_ms: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    simple_test().await;
    Ok(())
}



fn create_packet(keypair: &Keypair) -> SignedPacket {
    let packet = SignedPacket::builder()
        .txt(
            "_experiment".try_into().unwrap(),
            "dht-test".try_into().unwrap(),
            300,
        )
        .sign(&keypair)
        .unwrap();
    packet
}

async fn double_check_missing(pubkey: &PublicKey) {
    println!("  - Double check if key is really missing with a new pkarr client.");
    let client = Client::builder()
    .no_relays()
    .build()
    .unwrap();
    let packet = client.resolve_most_recent(pubkey).await;
    if packet.is_none() {
        println!("  - Packet for {pubkey} is indeed missing!")
    } else {
        println!("  - {pubkey} all good!!!")
    }
}

async fn simple_test() {
    let client = Client::builder()
        .no_relays()
        .cache_size(0)
        .maximum_ttl(0)
        .build()
        .unwrap();
    let mut keys: Vec<PublicKey> = vec![];
    for i in 0..100 {
        let keypair = Keypair::random();
        keys.push(keypair.public_key());
        let packet = create_packet(&keypair);
        client.publish(&packet, None).await.expect("Packet to be published successfully.");
        println!("- {i} Published {}", keypair.public_key());
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    loop {
        println!();
        println!("Start resolving keys.");
        for pubkey in keys.iter() {
            let packet = client.resolve_most_recent(pubkey).await;
            if packet.is_none() {
                println!("- Packet for {pubkey} missing!");
                double_check_missing(pubkey).await;
            } else {
                println!("- {pubkey} all good.")
            }
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }
    }
}