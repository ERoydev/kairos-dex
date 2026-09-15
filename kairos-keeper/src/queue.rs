use tokio::sync::{Mutex, mpsc::Receiver};

use crate::Task;
use crate::config;
use std::{collections::BinaryHeap, sync::Arc};

/// DataStructure represents the Pool with tasks using BinaryHeap,
/// its only purpose is to apply ordering based on priority,
/// otherwise i could just use tokio tasks and its thread pool
#[allow(unused)]
pub struct TaskQueue {
    queue: Arc<Mutex<BinaryHeap<Task>>>, // This is Max-Heap
    rx: Receiver<Task>,
    rpc_client: rpc::RpcClient,
}

impl TaskQueue {
    pub fn new(rx: Receiver<Task>, rpc_client: rpc::RpcClient) -> Self {
        Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            rx,
            rpc_client,
        }
    }

    pub async fn run(mut self) {
        let dispatch_queue = self.queue.clone(); // Arc clone, cheap
        let rpc_client = self.rpc_client.clone(); // reqwest::Client pools internally, cheap

        // Spawn a tokio task to handle tasks from TaskQueue and send them to tokio scheduler
        tokio::spawn(async move {
            loop {
                let maybe_task = dispatch_queue.lock().await.pop();
                if let Some(task) = maybe_task {
                    let rpc_client = rpc_client.clone();
                    tokio::spawn(async move {
                        let keeper_keypair = &config::get().keeper_keypair;
                        if let Err(e) = task.prepare_perp_tx(keeper_keypair, &rpc_client).await {
                            eprintln!("failed to prepare tx for task {}: {e}", task.id);
                        }
                    });
                }
            }
        });

        // receive messages, while runs as long as the channel stays open
        while let Some(new_task) = self.rx.recv().await {
            println!("Task received: {:?}", new_task);
            self.push(new_task).await;
        }
    }

    // ========== BinaryHeap Interface =========
    async fn push(&mut self, new_task: Task) {
        self.queue.lock().await.push(new_task);
    }

    // async fn pop(&mut self) -> Option<Task> {
    //     self.queue.lock().await.pop()
    // }

    // pub fn _len(&self) -> usize {
    //     self.queue.len()
    // }

    // pub fn _is_empty(&self) -> bool {
    //     self.queue.is_empty()
    // }
}
