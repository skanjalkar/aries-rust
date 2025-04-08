use aries_rust::{
    log_mod::{LogManager, LogRecordType},
    common::{PageID, TransactionID, Result},
    buffer::BufferManager,
    transaction::TransactionManager,
    storage::{File, FileMode},
};
use std::path::Path;
use std::sync::{Arc, Mutex};


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

#[test]
fn test_log_record_count() -> Result<()> {
    let log_path = Path::new("test_log_record.dat");
    let buffer_manager = Arc::new(Mutex::new(BufferManager::new(4096, 10)));
    let log_manager = Arc::new(Mutex::new(LogManager::new(log_path)?));
    let mut txn_manager = TransactionManager::new(
        Arc::clone(&log_manager),
        Arc::clone(&buffer_manager)
    );

    let txn_id = txn_manager.start_txn()?;
    
    // Add two update operations
    let page_id = PageID(1);
    let before_img = vec![0; 10];
    let after_img = vec![1; 10];
    
    log_manager.lock().unwrap().log_update(
        txn_id,
        page_id,
        10,  // length
        0,   // offset
        &before_img,
        &after_img
    )?;

    log_manager.lock().unwrap().log_update(
        txn_id,
        PageID(2),
        10,  // length
        0,   // offset
        &before_img,
        &after_img
    )?;
    
    txn_manager.commit_txn(txn_id)?;
    
    let log_count = log_manager.lock().unwrap().get_total_log_records();
    assert_eq!(log_count, 4); // Begin + 2 Updates + Commit
    
    let update_count = log_manager.lock().unwrap()
        .get_total_log_records_of_type(LogRecordType::UpdateRecord);
    assert_eq!(update_count, 2);
    
    std::fs::remove_file(log_path)?;
    Ok(())
}

#[test]
fn test_open_commit_open_crash() -> Result<()> {
    let log_path = Path::new("test_multi_txn_crash.dat");
    let buffer_manager = Arc::new(Mutex::new(BufferManager::new(4096, 10)));
    let log_manager = Arc::new(Mutex::new(LogManager::new(log_path)?));
    let mut txn_manager = TransactionManager::new(
        Arc::clone(&log_manager),
        Arc::clone(&buffer_manager)
    );

    // T1: Start but don't commit
    let txn1 = txn_manager.start_txn()?;
    // Insert operations...
    buffer_manager.lock().unwrap().flush_all_pages()?;

    // T2: Start and commit
    let txn2 = txn_manager.start_txn()?;
    // Insert operations...
    txn_manager.commit_txn(txn2)?;

    // T3: Start but don't commit
    let txn3 = txn_manager.start_txn()?;
    // Insert operations...
    buffer_manager.lock().unwrap().flush_all_pages()?;

    // Simulate crash
    buffer_manager.lock().unwrap().discard_all_pages()?;
    let new_log_file = File::open_file(log_path, FileMode::WRITE)?;
    log_manager.lock().unwrap().reset(new_log_file)?;

    // Verify only T2's data remains...

    std::fs::remove_file(log_path)?;
    Ok(())
}