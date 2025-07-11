use env_logger::Builder;
use log::{info, LevelFilter};
use std::path::Path;
use std::sync::{Arc, Mutex};

use aries_rust::{buffer::BufferManager, log_mod::LogManager, transaction::TransactionManager};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Builder::new().filter_level(LevelFilter::Info).init();

    info!("Aries Protocol Implementation in Rust");

    // Use a temporary file for the log - we'll clean it up at the end
    let log_path = Path::new("temp_log.dat");

    // Set up our core components with reasonable defaults
    let buffer_manager = Arc::new(Mutex::new(BufferManager::new(4096, 100))); // 4KB pages, 100 page buffer
    let log_manager = Arc::new(Mutex::new(LogManager::new(log_path)?));
    let mut txn_manager =
        TransactionManager::new(Arc::clone(&log_manager), Arc::clone(&buffer_manager));

    // Simple test: start a transaction and commit it
    let txn_id = txn_manager.start_txn()?;
    info!("Started transaction {}", txn_id.0);

    txn_manager.commit_txn(txn_id)?;
    info!("Committed transaction {}", txn_id.0);

    // Clean up our temp file
    if log_path.exists() {
        std::fs::remove_file(log_path)?;
    }

    Ok(())
}
