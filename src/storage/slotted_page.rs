use crate::common::{BuzzDBError, PageID, RecordID, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SlottedPage {
    pub page_id: PageID,
    pub slots: Vec<Option<RecordID>>, // Fixed-size array of record slots
}

impl SlottedPage {
    pub fn new(page_id: PageID, num_slots: usize) -> Self {
        Self {
            page_id,
            slots: vec![None; num_slots],
        }
    }

    pub fn allocate_slot(&mut self, record_id: RecordID) -> Option<usize> {
        // Linear search for first empty slot - not great for performance but simple
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(record_id);
                return Some(index);
            }
        }
        None // Page is full
    }

    pub fn deallocate_slot(&mut self, slot_index: usize) -> Result<()> {
        if slot_index >= self.slots.len() {
            return Err(BuzzDBError::InvalidSlotIndex(slot_index));
        }
        self.slots[slot_index] = None; // Mark slot as free
        Ok(())
    }

    pub fn get_record_id(&self, slot_index: usize) -> Result<RecordID> {
        if slot_index >= self.slots.len() {
            return Err(BuzzDBError::InvalidSlotIndex(slot_index));
        }

        match self.slots[slot_index] {
            Some(record_id) => Ok(record_id),
            None => Err(BuzzDBError::EmptySlot(slot_index)),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        // Using bincode for now - might want to switch to a more compact format later
        bincode::serialize(self).expect("Serialization failed")
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|_| BuzzDBError::DeserializationError)
    }
}
