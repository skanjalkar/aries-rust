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
use std::sync::Arc;
use std::sync::Mutex;

pub struct Database {
    files: DBFiles,
    buffer_manager: Arc<Mutex<BufferManager>>,
    log_manager: Arc<Mutex<LogManager>>,
    transaction_manager: TransactionManager,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        let files = DBFiles::new(db_path)?;
        
        let buffer_manager = Arc::new(Mutex::new(BufferManager::new(4096, 1000)));
        let log_manager = Arc::new(Mutex::new(LogManager::new(&files.get_log_file_path())?));
        
        let transaction_manager = TransactionManager::new(
            Arc::clone(&log_manager),
            Arc::clone(&buffer_manager)
        );

        Ok(Self {
            files,
            buffer_manager,
            log_manager,
            transaction_manager,
        })
    }
    
    pub fn begin_transaction(&mut self) -> Result<TransactionID> {
        self.transaction_manager.start_txn()
    }

    pub fn commit_transaction(&mut self, txn_id: TransactionID) -> Result<()> {
        self.transaction_manager.commit_txn(txn_id)
    }

    pub fn get_log_manager(&self) -> Arc<Mutex<LogManager>> {
        Arc::clone(&self.log_manager)
    }

    pub fn cleanup(&self) -> Result<()> {
        self.files.cleanup()
    }

    pub fn close(&mut self) -> Result<()> {
        // Flush buffer pool
        self.buffer_manager.lock().unwrap().flush_all_pages()?;
        
        // Cleanup files
        self.files.cleanup()?;
        
        Ok(())
    }
}