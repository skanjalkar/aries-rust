use aries_rust::{
    log_mod::{LogManager, LogRecordType},
    common::{PageID, TransactionID, Result},
    buffer::BufferManager,
};
use std::path::Path;

#[test]
fn test_log_manager_basic() -> Result<()> {
    let log_path = Path::new("test_log_basic.dat");
    let mut log_manager = LogManager::new(log_path)?;
    
    let txn_id = TransactionID(1);
    
    // Log different types of records
    log_manager.log_txn_begin(txn_id)?;
    
    let page_id = PageID(1);
    let before_img = vec![0; 10];
    let after_img = vec![1; 10];
    log_manager.log_update(txn_id, page_id, 10, 0, &before_img, &after_img)?;
    
    log_manager.log_commit(txn_id)?;
    
    // Verify log record counts
    assert_eq!(log_manager.get_total_log_records_of_type(LogRecordType::BeginRecord), 1);
    assert_eq!(log_manager.get_total_log_records_of_type(LogRecordType::UpdateRecord), 1);
    assert_eq!(log_manager.get_total_log_records_of_type(LogRecordType::CommitRecord), 1);
    
    // Clean up
    std::fs::remove_file(log_path).ok();
    Ok(())
}

#[test]
fn test_log_recovery() -> Result<()> {
    let log_path = Path::new("test_log_recovery.dat");
    let mut log_manager = LogManager::new(log_path)?;
    let mut buffer_manager = BufferManager::new(4096, 10);
    
    let txn_id = TransactionID(1);
    let page_id = PageID(1);
    
    // Create some log records
    log_manager.log_txn_begin(txn_id)?;
    
    let before_img = vec![0; 10];
    let after_img = vec![1; 10];
    log_manager.log_update(txn_id, page_id, 10, 0, &before_img, &after_img)?;
    
    // Simulate crash before commit
    log_manager.recovery(&mut buffer_manager)?;
    
    // Clean up
    std::fs::remove_file(log_path).ok();
    Ok(())
}