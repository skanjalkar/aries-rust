use aries_rust::{
    buffer::BufferManager,
    log_mod::LogManager,
    transaction::TransactionManager,
    common::{PageID, Result},
};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[test]
fn test_complete_workflow() -> Result<()> {
    // Set up components
    let log_path = Path::new("test_integration.dat");
    let buffer_manager = Arc::new(Mutex::new(BufferManager::new(4096, 10)));
    let log_manager = Arc::new(Mutex::new(LogManager::new(log_path)?));
    let mut txn_manager = TransactionManager::new(
        Arc::clone(&log_manager),
        Arc::clone(&buffer_manager)
    );
    // Start a transaction
    let txn_id = txn_manager.start_txn()?;
    
    // Modify some pages
    let pages = vec![PageID(1), PageID(2), PageID(3)];
    for page in pages {
        txn_manager.add_modified_page(txn_id, page)?;
    }
    
    // Commit the transaction
    txn_manager.commit_txn(txn_id)?;
    
    // Start another transaction and abort it
    let txn_id2 = txn_manager.start_txn()?;
    txn_manager.add_modified_page(txn_id2, PageID(4))?;
    txn_manager.abort_txn(txn_id2)?;
    
    // Clean up
    std::fs::remove_file(log_path).ok();
    Ok(())
}