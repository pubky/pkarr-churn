use mainline::Dht;
use pkarr::PublicKey;



/// Queries the public key and returns how many nodes responded with the packet.
pub fn count_dht_nodes_storing_packet(pubkey: &PublicKey, client: &Dht) -> u8 {
    let stream = client.get_mutable(pubkey.as_bytes(), None, None);
    let mut response_count: u8 = 0;
    for _ in stream {
        response_count += 1;
    }

    response_count
}