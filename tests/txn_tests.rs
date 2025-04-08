use aries_rust::{
    transaction::TransactionManager,
    log_mod::LogManager,
    buffer::BufferManager,
    common::{PageID, Result},
};
use std::path::Path;

#[test]
fn test_transaction_manager_concurrent() -> Result<()> {
    let log_path = Path::new("test_txn_concurrent.dat");
    let buffer_manager = BufferManager::new(4096, 10);
    let log_manager = LogManager::new(log_path)?;
    let mut txn_manager = TransactionManager::new(log_manager, buffer_manager);
    
    // Start multiple transactions
    let txn1 = txn_manager.start_txn()?;
    let txn2 = txn_manager.start_txn()?;
    
    // Modify pages in both transactions
    let page1 = PageID(1);
    let page2 = PageID(2);
    
    txn_manager.add_modified_page(txn1, page1)?;
    txn_manager.add_modified_page(txn2, page2)?;
    
    // Commit one transaction and abort another
    txn_manager.commit_txn(txn1)?;
    txn_manager.abort_txn(txn2)?;
    
    // Clean up
    std::fs::remove_file(log_path).ok();
    Ok(())
}

#[test]
fn test_transaction_isolation() -> Result<()> {
    let log_path = Path::new("test_txn_isolation.dat");
    let buffer_manager = BufferManager::new(4096, 10);
    let log_manager = LogManager::new(log_path)?;
    let mut txn_manager = TransactionManager::new(log_manager, buffer_manager);
    
    // Start two transactions
    let txn1 = txn_manager.start_txn()?;
    let txn2 = txn_manager.start_txn()?;
    
    // Both try to modify the same page
    let page1 = PageID(1);
    
    txn_manager.add_modified_page(txn1, page1)?;
    
    // Second transaction should fail to modify the same page
    assert!(txn_manager.add_modified_page(txn2, page1).is_err());
    
    // Clean up
    std::fs::remove_file(log_path).ok();
    Ok(())
}