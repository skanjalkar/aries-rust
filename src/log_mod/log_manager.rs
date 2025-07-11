use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::hash::Hash;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use crate::buffer::BufferManager;
use crate::common::{BuzzDBError, PageID, Result, TransactionID};

// Log record types - these need to match the on-disk format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogRecordType {
    BeginRecord = 0,
    CommitRecord = 1,
    AbortRecord = 2,
    UpdateRecord = 3,
    CheckpointRecord = 4,
}

impl From<u8> for LogRecordType {
    fn from(value: u8) -> Self {
        match value {
            0 => LogRecordType::BeginRecord,
            1 => LogRecordType::CommitRecord,
            2 => LogRecordType::AbortRecord,
            3 => LogRecordType::UpdateRecord,
            4 => LogRecordType::CheckpointRecord,
            _ => panic!("Invalid log record type: {}", value),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogRecordData {
    pub record_type: LogRecordType,
    pub txn_id: TransactionID,
    pub page_id: Option<PageID>,
    pub length: Option<u64>,
    pub offset: Option<u64>,
    pub before_img: Option<Vec<u8>>, // Before image for undo
    pub after_img: Option<Vec<u8>>,  // After image for redo
    pub log_offset: usize,
    pub record_size: usize,
}

pub struct LogManager {
    log_file: File,
    current_offset: usize, // Current write position in the log
    record_counts: HashMap<LogRecordType, u64>,
    txn_id_to_first_log_record: HashMap<TransactionID, usize>, // For rollback
}

impl LogManager {
    pub fn new(log_file_path: &Path) -> Result<Self> {
        let log_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(log_file_path)
            .map_err(BuzzDBError::IOError)?;

        Ok(Self {
            log_file,
            current_offset: 0,
            record_counts: HashMap::new(),
            txn_id_to_first_log_record: HashMap::new(),
        })
    }

    pub fn reset(&mut self, log_file: File) -> Result<()> {
        self.log_file = log_file;
        self.current_offset = 0;
        self.txn_id_to_first_log_record.clear();
        self.record_counts.clear();
        Ok(())
    }

    fn write_to_log(&mut self, data: &[u8]) -> Result<()> {
        self.log_file
            .seek(SeekFrom::Start(self.current_offset as u64))
            .map_err(BuzzDBError::IOError)?;

        self.log_file
            .write_all(data)
            .map_err(BuzzDBError::IOError)?;

        self.current_offset += data.len();
        Ok(())
    }

    pub fn log_txn_begin(&mut self, txn_id: TransactionID) -> Result<()> {
        let record_type = [LogRecordType::BeginRecord as u8];
        self.write_to_log(&record_type)?;

        let txn_id_bytes = txn_id.0.to_le_bytes();
        self.write_to_log(&txn_id_bytes)?;

        *self
            .record_counts
            .entry(LogRecordType::BeginRecord)
            .or_insert(0) += 1;

        // Track where this transaction's log records start (for rollback)
        self.txn_id_to_first_log_record
            .insert(txn_id, self.current_offset - std::mem::size_of::<u64>() - 1);

        Ok(())
    }

    pub fn log_commit(&mut self, txn_id: TransactionID) -> Result<()> {
        let record_type = [LogRecordType::CommitRecord as u8];
        self.write_to_log(&record_type)?;

        let txn_id_bytes = txn_id.0.to_le_bytes();
        self.write_to_log(&txn_id_bytes)?;

        *self
            .record_counts
            .entry(LogRecordType::CommitRecord)
            .or_insert(0) += 1;

        self.txn_id_to_first_log_record.remove(&txn_id);

        Ok(())
    }

    pub fn log_abort(
        &mut self,
        txn_id: TransactionID,
        buffer_manager: &mut BufferManager,
    ) -> Result<()> {
        let record_type = [LogRecordType::AbortRecord as u8];
        self.write_to_log(&record_type)?;

        let txn_id_bytes = txn_id.0.to_le_bytes();
        self.write_to_log(&txn_id_bytes)?;

        *self
            .record_counts
            .entry(LogRecordType::AbortRecord)
            .or_insert(0) += 1;

        // Actually perform the rollback by undoing changes
        self.rollback_txn(txn_id, buffer_manager)?;

        self.txn_id_to_first_log_record.remove(&txn_id);

        Ok(())
    }

    pub fn log_update(
        &mut self,
        txn_id: TransactionID,
        page_id: PageID,
        length: u64,
        offset: u64,
        before_img: &[u8],
        after_img: &[u8],
    ) -> Result<()> {
        let record_type = [LogRecordType::UpdateRecord as u8];
        self.write_to_log(&record_type)?;

        let txn_id_bytes = txn_id.0.to_le_bytes();
        let page_id_bytes = page_id.0.to_le_bytes();
        let length_bytes = length.to_le_bytes();
        let offset_bytes = offset.to_le_bytes();

        self.write_to_log(&txn_id_bytes)?;
        self.write_to_log(&page_id_bytes)?;
        self.write_to_log(&length_bytes)?;
        self.write_to_log(&offset_bytes)?;

        self.write_to_log(before_img)?;
        self.write_to_log(after_img)?;

        *self
            .record_counts
            .entry(LogRecordType::UpdateRecord)
            .or_insert(0) += 1;

        Ok(())
    }

    pub fn log_checkpoint(&mut self, _buffer_manager: &BufferManager) -> Result<()> {
        let record_type = [LogRecordType::CheckpointRecord as u8];
        self.write_to_log(&record_type)?;

        // TODO: Write dirty page table and active transaction table
        *self
            .record_counts
            .entry(LogRecordType::CheckpointRecord)
            .or_insert(0) += 1;

        Ok(())
    }

    pub fn recovery(&mut self, buffer_manager: &mut BufferManager) -> Result<()> {
        let logs = self.read_all_logs()?;

        // Classic ARIES three-phase recovery
        let (active_txns, committed_txns, aborted_txns) = self.analysis_phase(&logs);

        self.redo_phase(&logs, &committed_txns, &active_txns, buffer_manager)?;

        // Undo all transactions that didn't commit
        let transactions_to_undo: HashSet<TransactionID> =
            active_txns.union(&aborted_txns).cloned().collect();
        self.undo_phase(&logs, &transactions_to_undo, buffer_manager)?;

        Ok(())
    }

    fn analysis_phase(
        &self,
        logs: &[LogRecordData],
    ) -> (
        HashSet<TransactionID>,
        HashSet<TransactionID>,
        HashSet<TransactionID>,
    ) {
        let mut active_txns = HashSet::new();
        let mut committed_txns = HashSet::new();
        let mut aborted_txns = HashSet::new();

        for log in logs {
            match log.record_type {
                LogRecordType::BeginRecord => {
                    active_txns.insert(log.txn_id);
                }
                LogRecordType::CommitRecord => {
                    active_txns.remove(&log.txn_id);
                    committed_txns.insert(log.txn_id);
                }
                LogRecordType::AbortRecord => {
                    active_txns.remove(&log.txn_id);
                    aborted_txns.insert(log.txn_id);
                }
                _ => {}
            }
        }

        (active_txns, committed_txns, aborted_txns)
    }

    fn redo_phase(
        &self,
        logs: &[LogRecordData],
        committed_txns: &HashSet<TransactionID>,
        active_txns: &HashSet<TransactionID>,
        buffer_manager: &mut BufferManager,
    ) -> Result<()> {
        for log in logs {
            if log.record_type == LogRecordType::UpdateRecord {
                // Only redo committed transactions and active ones (they might have committed)
                if committed_txns.contains(&log.txn_id) || active_txns.contains(&log.txn_id) {
                    if let (Some(page_id), Some(offset), Some(after_img), Some(length)) =
                        (log.page_id, log.offset, &log.after_img, log.length)
                    {
                        let frame = buffer_manager.fix_page(page_id, true)?;
                        {
                            let mut frame_guard = frame.lock().unwrap();
                            let data = frame_guard.get_data_mut();

                            // Apply the "after" image to redo the change
                            data[offset as usize..offset as usize + length as usize]
                                .copy_from_slice(&after_img[0..length as usize]);
                        }

                        buffer_manager.unfix_page(Arc::clone(&frame), true)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn undo_phase(
        &self,
        logs: &[LogRecordData],
        transactions_to_undo: &HashSet<TransactionID>,
        buffer_manager: &mut BufferManager,
    ) -> Result<()> {
        // Process in reverse order to undo changes
        for log in logs.iter().rev() {
            if transactions_to_undo.contains(&log.txn_id) {
                if log.record_type == LogRecordType::UpdateRecord {
                    if let (Some(page_id), Some(offset), Some(before_img), Some(length)) =
                        (log.page_id, log.offset, &log.before_img, log.length)
                    {
                        let frame = buffer_manager.fix_page(page_id, true)?;
                        {
                            let mut frame_guard = frame.lock().unwrap();
                            let data = frame_guard.get_data_mut();

                            // Apply the "before" image to undo the change
                            data[offset as usize..offset as usize + length as usize]
                                .copy_from_slice(&before_img[0..length as usize]);
                        }

                        buffer_manager.unfix_page(Arc::clone(&frame), true)?;
                    }
                } else if log.record_type == LogRecordType::BeginRecord {
                    // Hit the beginning of the transaction - we're done
                    break;
                }
            }
        }

        Ok(())
    }

    fn rollback_txn(
        &self,
        txn_id: TransactionID,
        buffer_manager: &mut BufferManager,
    ) -> Result<()> {
        let logs = self.read_all_logs()?;

        for log in logs.iter().rev() {
            if log.txn_id == txn_id {
                if log.record_type == LogRecordType::UpdateRecord {
                    if let (Some(page_id), Some(offset), Some(before_img), Some(length)) =
                        (log.page_id, log.offset, &log.before_img, log.length)
                    {
                        let frame = buffer_manager.fix_page(page_id, true)?;
                        {
                            let mut frame_guard = frame.lock().unwrap();
                            let data = frame_guard.get_data_mut();

                            data[offset as usize..offset as usize + length as usize]
                                .copy_from_slice(&before_img[0..length as usize]);
                        }

                        buffer_manager.unfix_page(Arc::clone(&frame), true)?;
                    }
                } else if log.record_type == LogRecordType::BeginRecord {
                    break;
                }
            }
        }

        Ok(())
    }

    fn read_all_logs(&self) -> Result<Vec<LogRecordData>> {
        let mut logs = Vec::new();
        let mut offset = 0;

        let mut file = &self.log_file;
        file.seek(SeekFrom::Start(0))
            .map_err(BuzzDBError::IOError)?;

        while offset < self.current_offset {
            // Read the basic record header (type + transaction ID)
            let mut record_type_buf = [0u8; 1];
            file.read_exact(&mut record_type_buf)
                .map_err(BuzzDBError::IOError)?;
            let record_type = LogRecordType::from(record_type_buf[0]);

            let mut txn_id_buf = [0u8; 8];
            file.read_exact(&mut txn_id_buf)
                .map_err(BuzzDBError::IOError)?;
            let txn_id = TransactionID(u64::from_le_bytes(txn_id_buf));

            let mut log_record = LogRecordData {
                record_type,
                txn_id,
                page_id: None,
                length: None,
                offset: None,
                before_img: None,
                after_img: None,
                log_offset: offset,
                record_size: 1 + 8,
            };

            if record_type == LogRecordType::UpdateRecord {
                // Update records have extra data: page_id, length, offset, before/after images
                let mut page_id_buf = [0u8; 8];
                file.read_exact(&mut page_id_buf)
                    .map_err(BuzzDBError::IOError)?;
                let page_id = PageID(u64::from_le_bytes(page_id_buf));

                let mut length_buf = [0u8; 8];
                file.read_exact(&mut length_buf)
                    .map_err(BuzzDBError::IOError)?;
                let length = u64::from_le_bytes(length_buf);

                let mut offset_buf = [0u8; 8];
                file.read_exact(&mut offset_buf)
                    .map_err(BuzzDBError::IOError)?;
                let record_offset = u64::from_le_bytes(offset_buf);

                let mut before_img = vec![0u8; length as usize];
                file.read_exact(&mut before_img)
                    .map_err(BuzzDBError::IOError)?;

                let mut after_img = vec![0u8; length as usize];
                file.read_exact(&mut after_img)
                    .map_err(BuzzDBError::IOError)?;

                log_record.page_id = Some(page_id);
                log_record.length = Some(length);
                log_record.offset = Some(record_offset);
                log_record.before_img = Some(before_img);
                log_record.after_img = Some(after_img);
                log_record.record_size += 8 + 8 + 8 + 2 * length as usize;
            }

            let record_size = log_record.record_size;
            logs.push(log_record);
            offset += record_size;
        }

        Ok(logs)
    }

    pub fn get_total_log_records(&self) -> u64 {
        self.record_counts.values().sum()
    }

    pub fn get_total_log_records_of_type(&self, record_type: LogRecordType) -> u64 {
        *self.record_counts.get(&record_type).unwrap_or(&0)
    }
}
