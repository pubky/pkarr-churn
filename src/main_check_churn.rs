//! Publish and save the published public keys in a file
//! so they can be reused in other experiments.
//! 
//! Run with `cargo run --bin main_check_churn`.
//! 
//! 
//! - 26/35 Key 7s4rbuq3t5txrrdn51ghzci3euqbozgy5esqmusa7n7qrtgqtd4o unresolved
//! - 27/35 Key 6c46gfbqereetzbghcyjatzksrj6wpm4yp16xctzhh6tp8dzgpwy is resolvable on 2 nodes.
//! - 28/35 Key xk4apsmbqefpt4ehg9jspe4qnu59fsfmqhkjhesf5mtx5f79d48o unresolved
//! - 29/35 Key 9yfb19idcgnfa1baf73snk3ockwo4qqgr51qt5e1wjdb7kn9z4zy is resolvable on 2 nodes.
//! - 30/35 Key q7cz8mj7px6q3n7pout5gr4zsjxowr8ytjnpssqz5cnhnfkhmy6o unresolved
//! - 31/35 Key swqua8m6x8mypnt8uno7shy69a4snu3znr61ox4jbmcxysqur4no unresolved
//! - 32/35 Key m7ap9phxho7umyajhguxkn5z6jn3jxrniet355szyzhebirqkdco unresolved
//! - 33/35 Key nbg51dj1zaihn8o4dck1a31tcbkzksim787qxp17pohtd48w6aey is resolvable on 5 nodes.
//! - 34/35 Key rcp8so4wikho7gbdqbw1smyo64cjkf7ktuwwhyzbksnx7ois44fo unresolved


use helpers::count_dht_nodes_storing_packet;
use mainline::{Dht, DhtBuilder};
use pkarr::Keypair;
use published_key::PublishedKey;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    }, time::Duration,
};
use tracing::{ info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;
use rand::seq::SliceRandom;
use rand::rng;

mod helpers;
mod published_key;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("main_check_churn started.");
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

    println!("Read published_secrets.txt");
    let published_keys = read_keys();

    println!("Read {} keys", published_keys.len());

    run_churn_loop(published_keys, &ctrlc_pressed).await;

    Ok(())
}

/// reads keys from published_secrets.txt
fn read_keys() -> Vec<PublishedKey> {
    let secret_srs = std::fs::read_to_string("published_secrets.txt").expect("File not found");
    let keys = secret_srs.lines().map(|line| line.to_string()).collect::<Vec<_>>();
    keys.into_iter().map(|key| {
        let secret = hex::decode(key).expect("invalid hex");
        let secret: [u8; 32] = secret.try_into().unwrap();
        let key = Keypair::from_secret_key(&secret);
        PublishedKey::new(key)
    }).collect::<Vec<_>>()
}


async fn run_churn_loop(
    mut all_keys: Vec<PublishedKey>,
    ctrlc_pressed: &Arc<AtomicBool>,
) -> Vec<PublishedKey> {
    let client = DhtBuilder::default().request_timeout(Duration::from_millis(1000)).build().unwrap();
    client.clone().as_async().bootstrapped().await;
    let all_keys_count = all_keys.len();
    let mut rng = rng();
    loop {

        let churn_count = all_keys.iter().filter(|key| key.is_churned()).count();
        let churn_fraction = churn_count as f64 / all_keys.len() as f64;
        info!("Current churn fraction: {:.2}%", churn_fraction * 100.0);

        for (i, key) in all_keys.iter_mut().enumerate() {
            let pubkey = &key.public_key();
            let nodes_count = count_dht_nodes_storing_packet(pubkey, &client).await;
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
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        all_keys.shuffle(&mut rng);
        info!("Shuffle keys");

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
        if ctrlc_pressed.load(Ordering::Relaxed) {
            break
        }
        println!("---------------------\n")
    }
    all_keys
}


