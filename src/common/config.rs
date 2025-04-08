#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub page_size: usize,
    pub buffer_pool_size: usize,
    pub max_wal_size_mb: usize,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            page_size: 4096,
            buffer_pool_size: 1000,
            max_wal_size_mb: 64,
        }
    }
}