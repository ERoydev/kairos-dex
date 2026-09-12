use tokio::sync::mpsc::Receiver;

use crate::Task;
use std::collections::BinaryHeap;

/// DataStructure represents the Pool with tasks using BinaryHeap
#[allow(unused)]
#[derive(Debug)]
pub struct TaskQueue {
    queue: BinaryHeap<Task>, // This is Max-Heap
    rx: Receiver<Task>,
}

impl TaskQueue {
    pub fn new(rx: Receiver<Task>) -> Self {
        Self {
            queue: BinaryHeap::new(),
            rx,
        }
    }

    pub async fn run(&mut self) {
        self.tick().await;
    }

    async fn tick(&mut self) {
        while let Some(i) = self.rx.recv().await {
            println!("got = {:?}", i);
        }
    }

    // ========== BinaryHeap Interface =========
    pub fn _push(&mut self, new_task: Task) {
        self.queue.push(new_task);
    }

    pub fn _pop(&mut self) -> Option<Task> {
        self.queue.pop()
    }

    pub fn _len(&self) -> usize {
        self.queue.len()
    }

    pub fn _is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
