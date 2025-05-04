use sqlx::SqlitePool;

pub trait Repository{}

#[derive(Debug, Clone)]
pub struct SqliteRepository{
    pool: SqlitePool 
}

impl SqliteRepository{
    pub fn new(pool: SqlitePool) -> Self{
        Self{pool}
    }
}

impl Repository for SqliteRepository {}
