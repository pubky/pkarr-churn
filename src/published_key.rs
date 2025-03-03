use pkarr::{Keypair, PublicKey, SignedPacket, Timestamp};


#[derive(Debug, Clone)]
pub struct PublishedKey {
    pub key: Keypair,
    pub created_at: Timestamp,
    pub churned_at: Option<Timestamp>
}

impl PublishedKey {
    pub fn new(key: Keypair) -> Self {
        Self {
            key,
            created_at: Timestamp::now(),
            churned_at: None
        }
    }

    /// Random new keypair
    pub fn random() -> Self {
        Self {
            key: Keypair::random(),
            created_at: Timestamp::now(),
            churned_at: None
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.key.public_key()
    }

    pub fn is_churned(&self) -> bool {
        self.churned_at.is_some()
    }

    /// Sets the churned_at timestamp to now
    pub fn mark_as_churned(&mut self) {
        self.churned_at = Some(Timestamp::now());
    }

    /// Removes the churned_at timestamp and therefore marks the key as available.
    pub fn mark_as_available(&mut self) {
        self.churned_at = None;
    }
    pub fn create_packet(&self) -> SignedPacket {
        SignedPacket::builder()
            .txt(
                "_experiment".try_into().unwrap(),
                "dht-test".try_into().unwrap(),
                300,
            )
            .sign(&self.key).unwrap()
    }
}