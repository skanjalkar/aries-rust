use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;

use crate::common::{BuzzDBError, PageID, RecordID, Result, TransactionID};
use crate::storage::SlottedPage;

#[derive(Debug)]
struct PageInfo {
    page: SlottedPage,
    is_dirty: bool,
    last_accessed: Instant,
    modifying_txn: Option<TransactionID>, // Track which transaction is currently modifying this page
}

pub struct HeapSegment {
    file: File,
    page_size: usize,
    num_slots_per_page: usize,
    pages: HashMap<PageID, PageInfo>,    // In-memory page cache
    page_access_order: VecDeque<PageID>, // LRU tracking
    max_pages_in_memory: usize,
    next_page_id: u64,
    dirty_pages: HashSet<PageID>, // Pages that need to be written to disk
}

impl HeapSegment {
    pub fn new(
        file_path: &Path,
        page_size: usize,
        num_slots_per_page: usize,
        max_pages_in_memory: usize,
    ) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)
            .map_err(BuzzDBError::IOError)?;

        Ok(Self {
            file,
            page_size,
            num_slots_per_page,
            pages: HashMap::new(),
            page_access_order: VecDeque::new(),
            max_pages_in_memory,
            next_page_id: 0,
            dirty_pages: HashSet::new(),
        })
    }

    pub fn allocate_page(&mut self, txn_id: TransactionID) -> Result<PageID> {
        let page_id = PageID(self.next_page_id);
        self.next_page_id += 1;

        let new_page = SlottedPage::new(page_id, self.num_slots_per_page);

        // Write the new page to disk first (crash safety)
        let serialized = new_page.serialize();
        if serialized.len() > self.page_size {
            return Err(BuzzDBError::PageSizeExceeded(
                serialized.len(),
                self.page_size,
            ));
        }

        let offset = page_id.0 as u64 * self.page_size as u64;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(&serialized)?;

        // Add to in-memory cache
        self.cache_page(page_id, new_page, txn_id)?;

        Ok(page_id)
    }

    pub fn get_page(&mut self, page_id: PageID) -> Result<&SlottedPage> {
        self.ensure_page_loaded(page_id)?;
        self.update_page_access(page_id);
        Ok(&self.pages.get(&page_id).unwrap().page)
    }

    pub fn get_page_mut(
        &mut self,
        page_id: PageID,
        txn_id: TransactionID,
    ) -> Result<&mut SlottedPage> {
        self.ensure_page_loaded(page_id)?;

        // Check for conflicting transactions (simple locking mechanism)
        if let Some(modifying_txn) = self.pages.get(&page_id).unwrap().modifying_txn {
            if modifying_txn != txn_id {
                return Err(BuzzDBError::Other(format!(
                    "Page {} is being modified by transaction {}",
                    page_id.0, modifying_txn.0
                )));
            }
        }

        self.update_page_access(page_id);
        self.mark_page_dirty(page_id, txn_id);

        Ok(&mut self.pages.get_mut(&page_id).unwrap().page)
    }

    pub fn insert_record(
        &mut self,
        page_id: PageID,
        record_id: RecordID,
        txn_id: TransactionID,
    ) -> Result<usize> {
        let page = self.get_page_mut(page_id, txn_id)?;

        match page.allocate_slot(record_id) {
            Some(slot_index) => Ok(slot_index),
            None => Err(BuzzDBError::PageFull(page_id.0)),
        }
    }

    pub fn delete_record(
        &mut self,
        page_id: PageID,
        slot_index: usize,
        txn_id: TransactionID,
    ) -> Result<()> {
        let page = self.get_page_mut(page_id, txn_id)?;
        page.deallocate_slot(slot_index)
    }

    pub fn get_record(&mut self, page_id: PageID, slot_index: usize) -> Result<RecordID> {
        let page = self.get_page(page_id)?;
        page.get_record_id(slot_index)
    }

    pub fn commit_transaction(&mut self, txn_id: TransactionID) -> Result<()> {
        let mut pages_to_write = Vec::new();

        // Collect all pages modified by this transaction
        for (&page_id, info) in self.pages.iter() {
            if info.modifying_txn == Some(txn_id) {
                let serialized = info.page.serialize();
                pages_to_write.push((page_id, serialized));
            }
        }

        for (page_id, serialized) in pages_to_write {
            if serialized.len() > self.page_size {
                return Err(BuzzDBError::PageSizeExceeded(
                    serialized.len(),
                    self.page_size,
                ));
            }

            let offset = page_id.0 as u64 * self.page_size as u64;
            self.file.seek(SeekFrom::Start(offset))?;
            self.file.write_all(&serialized)?;

            if let Some(info) = self.pages.get_mut(&page_id) {
                info.modifying_txn = None;
                info.is_dirty = false;
            }
            self.dirty_pages.remove(&page_id);
        }

        self.file.sync_all()?;
        Ok(())
    }

    pub fn abort_transaction(&mut self, txn_id: TransactionID) -> Result<()> {
        let modified_pages: Vec<PageID> = self
            .pages
            .iter()
            .filter(|(_, info)| info.modifying_txn == Some(txn_id))
            .map(|(&page_id, _)| page_id)
            .collect();

        // Simple rollback: just reload from disk to discard changes
        for page_id in modified_pages {
            self.pages.remove(&page_id);
            self.ensure_page_loaded(page_id)?;
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        let mut pages_to_write = Vec::new();

        for &page_id in &self.dirty_pages {
            if let Some(info) = self.pages.get(&page_id) {
                let serialized = info.page.serialize();
                pages_to_write.push((page_id, serialized));
            }
        }

        for (page_id, serialized) in pages_to_write {
            if serialized.len() > self.page_size {
                return Err(BuzzDBError::PageSizeExceeded(
                    serialized.len(),
                    self.page_size,
                ));
            }

            let offset = page_id.0 as u64 * self.page_size as u64;
            self.file.seek(SeekFrom::Start(offset))?;
            self.file.write_all(&serialized)?;

            if let Some(info) = self.pages.get_mut(&page_id) {
                info.is_dirty = false;
            }
        }

        self.dirty_pages.clear();
        self.file.sync_all()?;
        Ok(())
    }

    fn read_page_from_disk(&mut self, page_id: PageID) -> Result<SlottedPage> {
        let offset = page_id.0 as u64 * self.page_size as u64;
        let mut buffer = vec![0; self.page_size];

        self.file.seek(SeekFrom::Start(offset))?;
        let bytes_read = self.file.read(&mut buffer)?;

        if bytes_read == 0 {
            return Err(BuzzDBError::PageNotFound(page_id.0));
        }

        SlottedPage::deserialize(&buffer[..bytes_read])
    }

    fn ensure_page_loaded(&mut self, page_id: PageID) -> Result<()> {
        if !self.pages.contains_key(&page_id) {
            if self.pages.len() >= self.max_pages_in_memory {
                self.evict_page()?;
            }

            let page = self.read_page_from_disk(page_id)?;
            let page_info = PageInfo {
                page,
                is_dirty: false,
                last_accessed: Instant::now(),
                modifying_txn: None,
            };

            self.pages.insert(page_id, page_info);
            self.page_access_order.push_back(page_id);
        }
        Ok(())
    }

    fn update_page_access(&mut self, page_id: PageID) {
        if let Some(info) = self.pages.get_mut(&page_id) {
            info.last_accessed = Instant::now();
        }

        if let Some(pos) = self.page_access_order.iter().position(|&p| p == page_id) {
            self.page_access_order.remove(pos);
            self.page_access_order.push_back(page_id);
        }
    }

    fn mark_page_dirty(&mut self, page_id: PageID, txn_id: TransactionID) {
        if let Some(info) = self.pages.get_mut(&page_id) {
            info.is_dirty = true;
            info.modifying_txn = Some(txn_id);
            self.dirty_pages.insert(page_id);
        }
    }

    fn evict_page(&mut self) -> Result<()> {
        while let Some(page_id) = self.page_access_order.pop_front() {
            // Can't evict dirty pages - they need to be written first
            if self.dirty_pages.contains(&page_id) {
                self.page_access_order.push_back(page_id);
                continue;
            }

            // Can't evict pages that are being modified by active transactions
            if let Some(info) = self.pages.get(&page_id) {
                if info.modifying_txn.is_some() {
                    self.page_access_order.push_back(page_id);
                    continue;
                }
            }

            self.pages.remove(&page_id);
            return Ok(());
        }

        // If we get here, everything is dirty or locked - we're stuck
        Err(BuzzDBError::BufferFull)
    }

    fn cache_page(
        &mut self,
        page_id: PageID,
        page: SlottedPage,
        txn_id: TransactionID,
    ) -> Result<()> {
        if self.pages.len() >= self.max_pages_in_memory {
            self.evict_page()?;
        }

        let page_info = PageInfo {
            page,
            is_dirty: true,
            last_accessed: Instant::now(),
            modifying_txn: Some(txn_id),
        };

        self.pages.insert(page_id, page_info);
        self.page_access_order.push_back(page_id);
        self.dirty_pages.insert(page_id);

        Ok(())
    }
}
