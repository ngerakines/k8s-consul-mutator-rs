use async_trait::async_trait;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;

use crate::error::Result;

#[async_trait]
pub trait KeyManager: Sync + Send {
    async fn watch(&self, key: String) -> Result<bool>;
    async fn set(&self, key: String, value: String) -> Result<()>;
    async fn get(&self, key: String) -> Result<Option<String>>;
}

pub struct NullKeyManager;

impl Default for NullKeyManager {
    fn default() -> Self {
        NullKeyManager
    }
}

#[async_trait]
impl KeyManager for NullKeyManager {
    async fn watch(&self, _key: String) -> Result<bool> {
        Ok(true)
    }

    async fn set(&self, _key: String, _value: String) -> Result<()> {
        Ok(())
    }

    async fn get(&self, _key: String) -> Result<Option<String>> {
        Ok(None)
    }
}

pub struct MemoryKeyManager {
    data: Mutex<RefCell<HashMap<String, String>>>,
}

impl Default for MemoryKeyManager {
    fn default() -> Self {
        Self {
            data: Mutex::new(Default::default()),
        }
    }
}

#[async_trait]
impl KeyManager for MemoryKeyManager {
    async fn watch(&self, _key: String) -> Result<bool> {
        Ok(true)
    }

    async fn set(&self, key: String, value: String) -> Result<()> {
        let data_lock = self.data.lock();
        let mut data = data_lock.borrow_mut();

        data.insert(key, value);

        Ok(())
    }

    async fn get(&self, state: String) -> Result<Option<String>> {
        let data_lock = self.data.lock();
        let data = data_lock.borrow_mut();

        match data.get(&state) {
            Some(val_ref) => Ok(Some(val_ref.to_owned())),
            None => Ok(None),
        }
    }
}
