use solana_sdk::{pubkey::Pubkey, signature::Signature};

#[derive(Debug)]
pub struct QueuePublisher {
    // TODO implemente redis client connection, this is going to be handled last
}

impl QueuePublisher {
    pub fn new() -> Self {
        QueuePublisher {}
    }

    pub async fn publish(&self, program_id: &Pubkey, signature: Signature) {
        println!(
            "Publish to redis queue with p_id: {}, and signature: {}",
            program_id, signature
        );
    }
}
