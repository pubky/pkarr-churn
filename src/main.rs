//! MRE to replicate the resolve bug.
//! This script publishes 100 pkarr packets and tries to resolve them after.
//! `resolve_most_recent()` can't find them. Only when a new pkarr client is created without any caching limitations, it is able to resolve.
//! `resolve()` itself is working better but also returning missing packets occasionally even though they should be there.
use std::time::Duration;

use clap::Parser;
use pkarr::{Client, Keypair, PublicKey, SignedPacket};


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

/// Double checks if the packet is really missing with a new pkarr client.
/// Usually it finds it this way.
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
        .cache_size(0) // Disable caching so we actually call the DHT.
        .maximum_ttl(0)
        .build()
        .unwrap();
    let mut keys: Vec<PublicKey> = vec![];

    // Publish 100 packets and save the public keys.
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
            // Resolve key with the same client.
            let packet = client.resolve_most_recent(pubkey).await;
            if packet.is_none() {
                println!("- Packet for {pubkey} missing!");
                double_check_missing(pubkey).await;
            } else {
                println!("- {pubkey} all good.")
            }
            // Sleep so we don't run into any rate limits. Not sure if this is needed.
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }
    }
}