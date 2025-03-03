use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, time::{Duration, Instant}};

use mainline::Dht;
use pkarr::{Client, PublicKey};

use crate::published_key::PublishedKey;



/// Queries the public key and returns how many nodes responded with the packet.
pub async fn count_dht_nodes_storing_packet(pubkey: &PublicKey, client: &Dht) -> u8 {
    let c = client.clone();
    let p = pubkey.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let stream = c.get_mutable(p.as_bytes(), None, None);
        let mut response_count: u8 = 0;
    
        for _ in stream {
            response_count += 1;
        }
    
        response_count
    });

    handle.await.unwrap()
}


// Publishes x number of packets. Checks if they are actually available
pub async fn publish_records(num_records: usize, thread_id: usize) -> Vec<PublishedKey> {
    let client = Client::builder().no_relays().cache_size(0).build().unwrap();
    let dht = mainline::Dht::client().unwrap();
    let mut records = vec![];

    for i in 0..num_records {
        let instant = Instant::now();
        let key = PublishedKey::random();
        let packet = key.create_packet();
        if let Err(e) = client.publish(&packet, None).await {
            tracing::error!("Failed to publish {} record: {e:?}", key.public_key());
            continue;
        }
        let publish_time = instant.elapsed().as_millis();
        let found_count = count_dht_nodes_storing_packet(&key.public_key(), &dht).await;
        tracing::info!("- t{thread_id} {i}/{num_records} Published {} on {found_count} nodes within {publish_time}ms", key.public_key());
        records.push(key);

        // if ctrlc_pressed.load(Ordering::Relaxed) {
        //     break
        // }
    }
    records
}