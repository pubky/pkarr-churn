use std::{fs, time::Instant};

use pkarr::{Client, Keypair, PublicKey, SignedPacket};

const NEW_COUNT: usize = 512;
const KEYS_FILE: &str = "keys.txt";

#[tokio::main]
async fn main() {
    let client = Client::builder().no_relays().build().unwrap();

    let mut keys = vec![];

    let signed_packet_builder = SignedPacket::builder().txt(
        "_experiment".try_into().unwrap(),
        "dht-test".try_into().unwrap(),
        3600,
    );

    for i in 0..NEW_COUNT {
        let keypair = Keypair::random();

        let start = Instant::now();

        match client
            .publish(&signed_packet_builder.clone().sign(&keypair).unwrap(), None)
            .await
        {
            Ok(_) => (),
            Err(_) => (),
        };

        let key = keypair.public_key();
        keys.push(key.clone());

        println!(
            "{}/{NEW_COUNT} Stored mutable data as {:?} in {:?} milliseconds",
            i + 1,
            key,
            start.elapsed().as_millis()
        );
    }

    save_keys_to_disk(&keys).expect("Failed to save keys to disk");
}

fn save_keys_to_disk(keys: &[PublicKey]) -> Result<(), std::io::Error> {
    let data = keys
        .iter()
        .map(|key| key.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(KEYS_FILE, data)
}
