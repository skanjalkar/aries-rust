use aries_rust::{
    transaction::TransactionManager,
    log_mod::LogManager,
    buffer::BufferManager,
    common::{PageID, Result},
};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[test]
fn test_transaction_manager_concurrent() -> Result<()> {
    let log_path = Path::new("test_txn_concurrent.dat");
    
    let buffer_manager = Arc::new(Mutex::new(BufferManager::new(4096, 10)));
    let log_manager = Arc::new(Mutex::new(LogManager::new(log_path)?));
    let mut txn_manager = TransactionManager::new(
        Arc::clone(&log_manager),
        Arc::clone(&buffer_manager)
    );
    
    // Start multiple transactions
    let txn1 = txn_manager.start_txn()?;
    let txn2 = txn_manager.start_txn()?;
    
    let page1 = PageID(1);
    let page2 = PageID(2);
    
    // We need to add back the add_modified_page method to TransactionManager
    txn_manager.add_modified_page(txn1, page1)?;
    txn_manager.add_modified_page(txn2, page2)?;
    
    // Commit one transaction and abort another
    txn_manager.commit_txn(txn1)?;
    txn_manager.abort_txn(txn2)?;
    
    // Clean up
    if log_path.exists() {
        std::fs::remove_file(log_path)?;
    }
    Ok(())
}

#[test]
fn test_transaction_isolation() -> Result<()> {
    let log_path = Path::new("test_txn_isolation.dat");
    let buffer_manager = Arc::new(Mutex::new(BufferManager::new(4096, 10)));
    let log_manager = Arc::new(Mutex::new(LogManager::new(log_path)?));
    let mut txn_manager = TransactionManager::new(
        Arc::clone(&log_manager),
        Arc::clone(&buffer_manager)
    );
    
    // Start two transactions
    let txn1 = txn_manager.start_txn()?;
    let txn2 = txn_manager.start_txn()?;
    
    // Both try to modify the same page
    let page1 = PageID(1);
    
    txn_manager.add_modified_page(txn1, page1)?;
    
    // Second transaction should fail to modify the same page
    assert!(txn_manager.add_modified_page(txn2, page1).is_err());

    // Clean up    
    if log_path.exists() {
        std::fs::remove_file(log_path)?;
    }
    Ok(())
}