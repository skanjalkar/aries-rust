pub mod buffer;
pub mod common;
pub mod heap;
pub mod log_mod;
pub mod storage;
pub mod transaction;

pub use buffer::BufferManager;
pub use common::{PageID, TransactionID, Result};
pub use log_mod::LogManager;
pub use transaction::TransactionManager;
pub use storage::DBFiles;

use std::path::Path;

pub struct Database {
    files: DBFiles,
    buffer_manager: BufferManager,
    log_manager: LogManager,
    transaction_manager: TransactionManager,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        // Initialize database files
        let files = DBFiles::new(db_path)?;
        
        // Initialize components with actual files
        let buffer_manager = BufferManager::new(4096, 1000);
        let log_manager = LogManager::new(&files.get_log_file_path())?;
        let transaction_manager = TransactionManager::new(log_manager, buffer_manager);

        Ok(Self {
            files,
            buffer_manager,
            log_manager,
            transaction_manager,
        })
    }

    pub fn open_database(db_path: &Path) -> Result<Self> {
        if !db_path.exists() {
            return Self::new(db_path);
        }
        
        // Initialize with existing files
        let files = DBFiles::new(db_path)?;
        let buffer_manager = BufferManager::new(4096, 1000);
        let log_manager = LogManager::new(&files.get_log_file_path())?;
        let transaction_manager = TransactionManager::new(log_manager, buffer_manager);

        Ok(Self {
            files,
            buffer_manager,
            log_manager,
            transaction_manager,
        })
    }

    pub fn close(&mut self) -> Result<()> {
        // Flush all changes and close files
        self.buffer_manager.flush_all_pages()?;
        Ok(())
    }
}