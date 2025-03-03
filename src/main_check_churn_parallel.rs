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


use clap::{command, Parser};
use helpers::count_dht_nodes_storing_packet;
use mainline::{Dht, DhtBuilder};
use pkarr::Keypair;
use published_key::PublishedKey;
use tokio::time::{sleep, Instant};
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

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Number of parallel threads
    #[arg(long, default_value_t = 1)]
    threads: usize,
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("main_check_churn_parallel started.");

    let cli = Cli::parse();

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

    run_churn_loop(published_keys, &ctrlc_pressed, cli.threads).await;

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
    thread_count: usize,
) -> Vec<PublishedKey> {

    let all_keys_count = all_keys.len();
    let mut rng = rng();
    all_keys.shuffle(&mut rng);

    let chunk_size = all_keys_count / thread_count;
    let chunks = all_keys.chunks(chunk_size).map(|chunk| chunk.to_vec()).collect::<Vec<_>>();

    let mut handles = vec![];
    for (thread_id, chunk) in chunks.into_iter().enumerate() {
        let handle = tokio::spawn(async move {
            check_chunks(chunk, thread_id).await
        });
        handles.push(handle);
    };

    let start = Instant::now();
    loop {
        let all_finished = handles
            .iter()
            .map(|handle| handle.is_finished())
            .reduce(|a, b| a && b)
            .unwrap();
        if all_finished {
            break;
        }
        if ctrlc_pressed.load(Ordering::Relaxed) {
            break;
        }
        sleep(Duration::from_millis(250)).await;
    }

    let passed = start.elapsed().as_secs();
    let rate = all_keys_count as f64 / passed as f64;
    tracing::info!("Resolved {all_keys_count} keys in {passed}s at {rate:.2} keys/s");

    if ctrlc_pressed.load(Ordering::Relaxed) {
        std::process::exit(0);
    }
    

    all_keys
}


async fn check_chunks(mut chunk: Vec<PublishedKey>, thread_id: usize) {
    let client = DhtBuilder::default().request_timeout(Duration::from_millis(1000)).build().unwrap();
    client.clone().as_async().bootstrapped().await;

    let keys_count = chunk.len();
    for (i, key) in chunk.iter_mut().enumerate() {
        let pubkey = &key.public_key();
        let nodes_count = count_dht_nodes_storing_packet(pubkey, &client).await;
        // Try to resolve the key.
        if nodes_count > 0 {
            info!("- t{thread_id:<3} {i:>2}/{} Key {pubkey} is resolvable on {nodes_count} nodes.", keys_count);
            key.mark_as_available();
        } else {
            info!("- t{thread_id:<3} {i:>2}/{} Key {pubkey} unresolved", keys_count);
            key.mark_as_churned();
        }
    }
}


