use async_trait::async_trait;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

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

#[derive(Default)]
struct InnerMemoryKeyManager {
    data: HashMap<String, String>,
    keys: HashSet<String>,
}

#[derive(Default)]
pub struct MemoryKeyManager {
    inner: Mutex<RefCell<InnerMemoryKeyManager>>,
}

#[async_trait]
impl KeyManager for MemoryKeyManager {
    async fn watch(&self, key: String) -> Result<bool> {
        let inner_lock = self.inner.lock();
        let mut inner = inner_lock.borrow_mut();

        if inner.data.contains_key(&key) {
            return Ok(false);
        }
        if inner.keys.contains(&key) {
            return Ok(false);
        }
        inner.keys.insert(key);

        Ok(true)
    }

    async fn set(&self, key: String, value: String) -> Result<()> {
        let inner_lock = self.inner.lock();
        let mut inner = inner_lock.borrow_mut();

        inner.data.insert(key, value);

        Ok(())
    }

    async fn get(&self, state: String) -> Result<Option<String>> {
        let inner_lock = self.inner.lock();
        let inner = inner_lock.borrow();

        match inner.data.get(&state) {
            Some(val_ref) => Ok(Some(val_ref.to_owned())),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_manager::NullKeyManager;

    #[tokio::test]
    async fn null_key_manager() {
        let key_manager = Box::new(NullKeyManager::default()) as Box<dyn KeyManager>;
        let watch_res = key_manager.watch("test".to_string()).await;
        assert!(watch_res.is_ok());
        assert_eq!(watch_res.unwrap(), true);
    }

    #[tokio::test]
    async fn memory_key_manager() {
        let key_manager = Box::new(MemoryKeyManager::default()) as Box<dyn KeyManager>;
        {
            let watch_res = key_manager.watch("test".to_string()).await;
            assert!(watch_res.is_ok());
            assert_eq!(watch_res.unwrap(), true);
        }
        {
            let watch_res2 = key_manager.watch("test".to_string()).await;
            assert!(watch_res2.is_ok());
            assert_eq!(watch_res2.unwrap(), false);
        }
    }
}
