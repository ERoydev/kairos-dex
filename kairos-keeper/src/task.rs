use std::cmp::{Ordering, Reverse};

use crate::now_unix;

/// For a keeper hittin Solana RPC, 3 is sane deafult,
/// since after some time this taks is not usefull anymore
/// as it can get liquidated by someone else after a few failed attempts
const MAX_RETRIES_LIQUIDATION: u32 = 3;

/// This tolerate more since it's not racing anyone
const MAX_RETRIES_FUNDING: u32 = 5;

#[derive(Debug, Eq, PartialEq)]
pub enum TaskType {
    FundingTick,      // market id
    LiquidationCheck, // position id
}

impl TaskType {
    pub fn get_priority(&self) -> Priority {
        match self {
            TaskType::FundingTick => Priority::Low,
            TaskType::LiquidationCheck => Priority::High,
        }
    }

    pub fn get_max_retries(&self) -> u32 {
        match self {
            TaskType::FundingTick => MAX_RETRIES_FUNDING,
            TaskType::LiquidationCheck => MAX_RETRIES_LIQUIDATION,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Priority {
    High,
    Low,
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Priority::High, Priority::Low) => Ordering::Greater,
            (Priority::High, Priority::High) => Ordering::Equal,

            (Priority::Low, Priority::High) => Ordering::Less,
            (Priority::Low, Priority::Low) => Ordering::Equal,
        }
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl Ord for Task {
    /// So if priority is equal, we use created_at to sort
    ///  take the smallest created_at value as higher priority
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| Reverse(self.created_at).cmp(&Reverse(other.created_at)))
    }
}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Represents a task in the pool of tasks for workers, ordering is implemented with (priority, created_at)
#[derive(Debug, Eq, PartialEq)]
pub struct Task {
    pub id: String,
    pub task_type: TaskType,
    pub priority: Priority,
    pub max_retries: u32,
    pub retries: u32,
    pub status: TaskStatus,
    pub created_at: i64,
    pub error: String,
}

impl Task {
    pub fn new(id: &str, task_type: TaskType) -> Self {
        let priority = task_type.get_priority();
        let max_retries = task_type.get_max_retries();

        Task {
            id: id.to_string(),
            task_type,
            priority,
            max_retries,
            retries: 0,
            status: TaskStatus::Pending,
            created_at: now_unix(),
            error: String::new(),
        }
    }
}
