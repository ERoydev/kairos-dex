use std::collections::HashMap;

use kairos_db::{DatabaseConnection, ingestor_cursor::Model as CursorModel};

#[derive(Debug)]
pub enum Cursor {
    Existing(CursorModel),
    Empty(EmptyCursor),
}

#[derive(Debug)]
pub struct EmptyCursor {}

pub struct CursorManager {
    db_connection: DatabaseConnection,
}

impl CursorManager {
    pub fn new(db: DatabaseConnection) -> Self {
        CursorManager { db_connection: db }
    }

    pub async fn load(&self, program_ids: Vec<String>) -> HashMap<String, Cursor> {
        let mut cursor_data: HashMap<String, Cursor> = HashMap::new();
        for p_id in program_ids {
            let data = match kairos_db::load_cursor(&self.db_connection, &p_id)
                .await
                .expect("Db connection should be ready")
            {
                Some(model) => Cursor::Existing(model),
                None => Cursor::Empty(EmptyCursor {}),
            };
            cursor_data.insert(p_id, data);
        }
        cursor_data
    }

    pub fn update(&self, program_id: String, sig: String) {}
}
