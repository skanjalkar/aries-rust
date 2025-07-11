use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::vec::Vec;

use crate::common::{BuzzDBError, PageID, Result};

pub struct BufferFrame {
    page_id: PageID,
    data: Vec<u8>,
    is_dirty: bool,
    pin_count: u32, // Reference count - can't evict while > 0
}

impl BufferFrame {
    pub fn new(page_id: PageID, page_size: usize) -> Self {
        Self {
            page_id: page_id,
            data: vec![0; page_size],
            is_dirty: false,
            pin_count: 0,
        }
    }

    pub fn get_data(&self) -> &[u8] {
        &self.data
    }

    pub fn get_data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.is_dirty = dirty;
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn pin(&mut self) {
        self.pin_count += 1;
    }

    pub fn unpin(&mut self) -> Result<()> {
        if self.pin_count == 0 {
            return Err(BuzzDBError::Other(
                "Cannot unpin a page with pin count 0".to_string(),
            ));
        }
        self.pin_count -= 1;
        Ok(())
    }

    pub fn pin_count(&self) -> u32 {
        self.pin_count
    }

    pub fn get_page_id(&self) -> PageID {
        self.page_id
    }
}

pub struct BufferManager {
    frames: HashMap<PageID, Arc<Mutex<BufferFrame>>>,
    page_size: usize,
    capacity: usize,
}

impl BufferManager {
    pub fn new(page_size: usize, capacity: usize) -> Self {
        Self {
            frames: HashMap::with_capacity(capacity),
            page_size,
            capacity,
        }
    }

    pub fn fix_page(
        &mut self,
        page_id: PageID,
        is_exclusive: bool,
    ) -> Result<Arc<Mutex<BufferFrame>>> {
        // Fast path: page already in buffer
        if let Some(frame) = self.frames.get(&page_id) {
            let mut frame = frame.lock().unwrap();
            frame.pin();
            return Ok(Arc::clone(&self.frames[&page_id]));
        }

        // Check if we need to make room and can actually do so
        if self.frames.len() >= self.capacity && !self.can_evict_page() {
            return Err(BuzzDBError::BufferFull);
        }

        if self.frames.len() >= self.capacity {
            self.evict_page()?;
        }

        let frame = BufferFrame::new(page_id, self.page_size);
        let frame = Arc::new(Mutex::new(frame));

        {
            let mut frame_guard = frame.lock().unwrap();
            frame_guard.pin();

            // Mark dirty upfront for exclusive access to avoid WAL issues
            if is_exclusive {
                frame_guard.set_dirty(true);
            }
        }

        self.frames.insert(page_id, Arc::clone(&frame));

        Ok(frame)
    }

    pub fn unfix_page(&mut self, frame: Arc<Mutex<BufferFrame>>, is_dirty: bool) -> Result<()> {
        let mut frame = frame.lock().unwrap();

        if is_dirty {
            frame.set_dirty(true);
        }

        frame.unpin()?;

        Ok(())
    }

    pub fn flush_page(&mut self, page_id: PageID) -> Result<()> {
        if let Some(frame) = self.frames.get(&page_id) {
            let frame = frame.lock().unwrap();
            if frame.is_dirty() {
                // TODO: Write page to disk
                // For now we're just pretending it worked
            }
        }

        Ok(())
    }

    pub fn flush_all_pages(&mut self) -> Result<()> {
        let page_ids: Vec<PageID> = self.frames.keys().copied().collect();

        for page_id in page_ids {
            self.flush_page(page_id)?;
        }

        Ok(())
    }

    pub fn discard_page(&mut self, page_id: PageID) -> Result<()> {
        if let Some(frame) = self.frames.get(&page_id) {
            let frame = frame.lock().unwrap();
            if frame.pin_count() > 0 {
                return Err(BuzzDBError::Other(
                    "Cannot discard a pinned page".to_string(),
                ));
            }
        }

        self.frames.remove(&page_id);
        Ok(())
    }

    pub fn discard_all_pages(&mut self) -> Result<()> {
        self.frames.retain(|_, frame| {
            let frame = frame.lock().unwrap();
            frame.pin_count() > 0
        });

        Ok(())
    }

    fn can_evict_page(&self) -> bool {
        for frame in self.frames.values() {
            let frame = frame.lock().unwrap();
            if frame.pin_count() == 0 {
                return true;
            }
        }

        false
    }

    fn evict_page(&mut self) -> Result<()> {
        // Simple eviction: grab the first unpinned page we find
        // TODO: Implement proper LRU replacement policy
        let page_id_to_evict = {
            let mut page_id_to_evict = None;

            for (page_id, frame) in &self.frames {
                let frame = frame.lock().unwrap();
                if frame.pin_count() == 0 {
                    page_id_to_evict = Some(*page_id);
                    break;
                }
            }

            page_id_to_evict
        };

        if let Some(page_id) = page_id_to_evict {
            self.flush_page(page_id)?;

            self.frames.remove(&page_id);

            return Ok(());
        }

        Err(BuzzDBError::BufferFull)
    }

    pub fn get_page_size(&self) -> usize {
        self.page_size
    }

    // Page ID encoding: top 16 bits = segment ID, bottom 48 bits = page ID
    pub fn get_overall_page_id(segment_id: u16, page_id: u64) -> PageID {
        PageID((segment_id as u64) << 48 | page_id)
    }

    pub fn get_segment_id(overall_page_id: PageID) -> u16 {
        (overall_page_id.0 >> 48) as u16
    }

    pub fn get_segment_page_id(overall_page_id: PageID) -> u64 {
        overall_page_id.0 & 0x0000FFFFFFFFFFFF
    }
}
