use std::{
    convert::TryFrom,
    fs,
    path::Path,
    thread::sleep,
    time::{Duration, SystemTime},
};

use pkarr::{Client, PublicKey};
use tracing::Level;
use tracing_subscriber;

const KEYS_FILE: &str = "keys.txt";
const SAMPLE_PER_SECONDS: u64 = 1;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let client = Client::builder().no_relays().cache_size(0).build().unwrap();

    let (keys, publish_time) = load_keys_from_disk().unwrap();

    let mut attempts = 0;
    let mut success = 0;

    loop {
        attempts += 1;

        // Sample a key
        let mut bytes = [0; 8];
        getrandom::fill(&mut bytes).expect("getrandom");

        let index = u64::from_le_bytes(bytes) as usize % keys.len();
        let key = keys[index].clone();

        if client.resolve(&key).await.is_some() {
            success += 1;

            println!("{}/{} Successfully resolved {}", success, attempts, key,);
        } else {
            println!(
                "{}/{} Failed to resolve a key {} after {} seconds of publishing.",
                attempts - success,
                attempts,
                key,
                publish_time.elapsed().unwrap().as_secs(),
            );
        };

        sleep(Duration::from_secs(SAMPLE_PER_SECONDS));
    }
}

fn load_keys_from_disk() -> Result<(Vec<PublicKey>, SystemTime), std::io::Error> {
    let path = Path::new(KEYS_FILE);

    if !path.exists() {
        panic!("Can NOT find the keys file");
    }

    let metadata = fs::metadata(path)?;
    let creation_time = metadata.created().unwrap();

    let data = fs::read_to_string(path)?;
    let keys = data
        .lines()
        .map(|str| PublicKey::try_from(str).unwrap())
        .collect();

    Ok((keys, creation_time))
}
