use std::fs;
use std::path::{Path, PathBuf};
use crate::common::Result;

pub struct DBFiles {
    db_directory: PathBuf,
    data_directory: PathBuf,
    log_directory: PathBuf,
    catalog_directory: PathBuf,
}

impl DBFiles {
    pub fn new(db_path: &Path) -> Result<Self> {
        let db_directory = db_path.to_path_buf();
        let data_directory = db_directory.join("data");
        let log_directory = db_directory.join("log");
        let catalog_directory = db_directory.join("catalog");

        // Create directories if they don't exist
        fs::create_dir_all(&data_directory)?;
        fs::create_dir_all(&log_directory)?;
        fs::create_dir_all(&catalog_directory)?;

        Ok(Self {
            db_directory,
            data_directory,
            log_directory,
            catalog_directory,
        })
    }

    pub fn get_data_file_path(&self, segment_id: u16) -> PathBuf {
        self.data_directory.join(format!("segment_{}.dat", segment_id))
    }

    pub fn get_log_file_path(&self) -> PathBuf {
        self.log_directory.join("wal.log")
    }

    pub fn get_catalog_file_path(&self) -> PathBuf {
        self.catalog_directory.join("catalog.dat")
    }

    pub fn cleanup(&self) -> Result<()> {
        if self.db_directory.exists() {
            fs::remove_dir_all(&self.db_directory)?;
        }
        Ok(())
    }
}