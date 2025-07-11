use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::buffer::BufferManager;
use crate::common::{BuzzDBError, PageID, Result, TransactionID};
use crate::log_mod::LogManager;

#[derive(Debug)]
pub struct Transaction {
    pub id: TransactionID,
    pub started: bool,
    pub modified_pages: HashSet<PageID>, // Pages we've written to
    pub locked_pages: HashSet<PageID>,   // Pages we're holding locks on
}

impl Transaction {
    pub fn new(id: TransactionID) -> Self {
        Self {
            id,
            started: true,
            modified_pages: HashSet::new(),
            locked_pages: HashSet::new(),
        }
    }

    pub fn add_modified_page(&mut self, page_id: PageID) {
        self.modified_pages.insert(page_id);
    }

    pub fn add_locked_page(&mut self, page_id: PageID) {
        self.locked_pages.insert(page_id);
    }

    pub fn remove_locked_page(&mut self, page_id: PageID) {
        self.locked_pages.remove(&page_id);
    }
}

pub struct TransactionManager {
    next_txn_id: u64,
    active_transactions: HashMap<TransactionID, Transaction>,
    page_locks: HashMap<PageID, TransactionID>, // Simple page-level locking
    log_manager: Arc<Mutex<LogManager>>,
    buffer_manager: Arc<Mutex<BufferManager>>,
}

impl TransactionManager {
    pub fn new(
        log_manager: Arc<Mutex<LogManager>>,
        buffer_manager: Arc<Mutex<BufferManager>>,
    ) -> Self {
        Self {
            next_txn_id: 0,
            active_transactions: HashMap::new(),
            page_locks: HashMap::new(),
            log_manager,
            buffer_manager,
        }
    }

    pub fn start_txn(&mut self) -> Result<TransactionID> {
        let txn_id = TransactionID(self.next_txn_id);
        self.next_txn_id += 1;

        let txn = Transaction::new(txn_id);
        self.active_transactions.insert(txn_id, txn);

        self.log_manager.lock().unwrap().log_txn_begin(txn_id)?;

        Ok(txn_id)
    }

    pub fn commit_txn(&mut self, txn_id: TransactionID) -> Result<()> {
        if let Some(txn) = self.active_transactions.remove(&txn_id) {
            let mut buffer_manager = self.buffer_manager.lock().unwrap();

            // Force all dirty pages to disk before committing
            for page_id in &txn.modified_pages {
                buffer_manager.flush_page(*page_id)?;
            }

            // Release all locks held by this transaction
            for page_id in &txn.locked_pages {
                self.page_locks.remove(page_id);
            }

            self.log_manager.lock().unwrap().log_commit(txn_id)?;
        } else {
            return Err(BuzzDBError::Other(format!(
                "Transaction {} not found",
                txn_id.0
            )));
        }

        Ok(())
    }

    pub fn abort_txn(&mut self, txn_id: TransactionID) -> Result<()> {
        if let Some(txn) = self.active_transactions.remove(&txn_id) {
            let mut buffer_manager = self.buffer_manager.lock().unwrap();
            let mut log_manager = self.log_manager.lock().unwrap();

            for page_id in &txn.modified_pages {
                buffer_manager.discard_page(*page_id)?;
            }

            for page_id in &txn.locked_pages {
                self.page_locks.remove(page_id);
            }

            log_manager.log_abort(txn_id, &mut buffer_manager)?;
        } else {
            return Err(BuzzDBError::Other(format!(
                "Transaction {} not found",
                txn_id.0
            )));
        }

        Ok(())
    }

    pub fn add_modified_page(&mut self, txn_id: TransactionID, page_id: PageID) -> Result<()> {
        // Check if someone else already has this page locked
        if let Some(&lock_holder) = self.page_locks.get(&page_id) {
            if lock_holder != txn_id {
                return Err(BuzzDBError::Other(format!(
                    "Page {} is locked by transaction {}",
                    page_id.0, lock_holder.0
                )));
            }
        }

        if let Some(txn) = self.active_transactions.get_mut(&txn_id) {
            // Grab the lock and track it
            self.page_locks.insert(page_id, txn_id);
            txn.add_locked_page(page_id);
            txn.add_modified_page(page_id);
            Ok(())
        } else {
            Err(BuzzDBError::Other(format!(
                "Transaction {} not found",
                txn_id.0
            )))
        }
    }
}
