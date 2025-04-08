use aries_rust::{
    buffer::BufferManager,
    log_mod::LogManager,
    transaction::TransactionManager,
    storage::{MemoryFile, FileMode, File},
    common::Result,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[test]
fn test_file_operations() {
    // Create a memory file and test basic operations
    let mut file = MemoryFile::new(FileMode::WRITE);
    
    // Test writing
    file.write_block(b"hello world", 0).unwrap();
    assert_eq!(file.size().unwrap(), 11);
    
    // Test reading
    let data = file.read_block(0, 5).unwrap();
    assert_eq!(&data, b"hello");
}

#[test]
fn test_transaction_lifecycle() -> Result<()> {
    // Create temporary log file
    let log_path = Path::new("test_log.dat");
    
    let buffer_manager = Arc::new(Mutex::new(BufferManager::new(4096, 10)));
    let log_manager = Arc::new(Mutex::new(LogManager::new(log_path)?));
    let mut txn_manager = TransactionManager::new(
        Arc::clone(&log_manager),
        Arc::clone(&buffer_manager)
    );
    
    // Start a transaction
    let txn_id = txn_manager.start_txn()?;
    
    // Commit it
    txn_manager.commit_txn(txn_id)?;
    
    if log_path.exists() {
        std::fs::remove_file(log_path)?;
    }
    Ok(())
}