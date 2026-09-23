// On startup, fetches signatures since cursor via RPC and pushes them to the queue.

use kairos_db::DatabaseConnection;

use crate::{Cursor, CursorManager};

/// StartUp Mechanism: make sure no events are missed between "where i stopped last time" and "now".
/// Cursor exists → fetch from cursor → now.
/// Cursor is empty (first run) → apply first-run policy (start from now, or backfill all).
pub struct StartUpCatchUpWorker {
    pub cursor_manager: CursorManager,
}

impl StartUpCatchUpWorker {
    pub fn new(db: DatabaseConnection) -> Self {
        StartUpCatchUpWorker {
            cursor_manager: CursorManager::new(db),
        }
    }

    pub async fn backfill(&self) {
        let program_ids = Vec::new();
        let cursors = self.cursor_manager.load(program_ids).await;
        for (program_id, cursor) in cursors {
            match cursor {
                Cursor::Existing(model) => {
                    println!("Model {:?}", model);
                    // cursor exists — resume from model.last_signature (or whatever field holds it)
                }
                Cursor::Empty(_) => {
                    // first run — apply first-run policy (start from now, or backfill all)
                }
            }
        }
        // for program_id in cursors {
        //     sigs =
        // }
        // for program_id in ["perp", "lp"]:
        //     sigs = rpc.get_signatures(program_id, cursors[program_id])
        //     for sig in sigs:
        //         queue_publisher.publish(program_id, sig)
    }
}
