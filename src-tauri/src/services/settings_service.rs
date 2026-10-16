use std::sync::{Arc, RwLock};

use crate::models::settings::Settings;
use crate::services::database_service::DatabaseService;

/// In-memory cached settings shared across threads.
///
/// Reads (monitor loop, AI worker) hit the cache; writes go through SQLite
/// and refresh the cache atomically so all threads observe updates.
#[derive(Clone)]
pub struct SettingsService {
    cache: Arc<RwLock<Settings>>,
    db: Arc<DatabaseService>,
}

impl SettingsService {
    pub fn new(db: Arc<DatabaseService>) -> Result<Self, String> {
        let initial = db.load_settings()?;
        Ok(Self {
            cache: Arc::new(RwLock::new(initial)),
            db,
        })
    }

    /// Cheap snapshot for hot paths (monitor loop / AI worker).
    pub fn get(&self) -> Settings {
        self.cache.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Read-modify-write via closure, persist, refresh cache.
    ///
    /// The write lock is held across the whole sequence: releasing it
    /// between the read and the persist let two concurrent updates clobber
    /// each other (lost update).
    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
        let mut guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
        let mut next = guard.clone();
        f(&mut next);
        self.db.persist_settings(&next)?;
        *guard = next.clone();
        Ok(next)
    }

    /// Replace all settings (frontend save).
    pub fn replace(&self, next: Settings) -> Result<Settings, String> {
        let mut guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
        self.db.persist_settings(&next)?;
        *guard = next.clone();
        Ok(next)
    }
}
