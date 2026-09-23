use std::sync::Arc;

use solana_sdk::{pubkey::Pubkey, signature::Signature};

use crate::{ProgramKind, Subscriber, health::HealthState};

#[derive(Debug)]
pub struct QueuePublisher {
    // TODO implemente redis client connection, this is going to be handled last
    pub program_ids: Vec<Pubkey>,
    pub health_state: Arc<HealthState>,
}

impl QueuePublisher {
    pub fn new(program_ids: Vec<Pubkey>) -> Self {
        QueuePublisher {
            program_ids,
            health_state: Arc::new(HealthState::new()),
        }
    }

    pub async fn run(&self) {
        // Start streaming subscription to listen for programs
        for p_id in self.program_ids.clone() {
            Subscriber::new(p_id, ProgramKind::Perp)
                .with_health_state(self.health_state.clone())
                .spawn();
        }
    }

    pub async fn publish(&self, program_id: &Pubkey, signature: Signature) {
        println!(
            "Publish to redis queue with p_id: {}, and signature: {}",
            program_id, signature
        );
    }
}
