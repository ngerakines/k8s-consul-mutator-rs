use async_trait::async_trait;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::vec;

use anyhow::anyhow;

use crate::error::Result;

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
pub struct Subscription {
    pub namespace: String,
    pub deployment: String,
    pub config_key: String,
}

#[async_trait]
pub trait KeyManager: Sync + Send {
    async fn watch(
        &self,
        namespace: String,
        deployment: String,
        config_key: String,
        consul_key: String,
    ) -> Result<bool>;
    async fn unwatch_namespace(&self, namespace: String) -> Result<usize>;
    async fn unwatch_deployment(&self, namespace: String, deployment: String) -> Result<usize>;
    async fn set(&self, key: String, value: String) -> Result<()>;
    async fn get(&self, key: String) -> Result<Option<String>>;
    async fn subscriptions_for_deployment(
        &self,
        namespace: String,
        deployment: String,
    ) -> Result<Vec<Subscription>>;
}

pub struct NullKeyManager;

impl Default for NullKeyManager {
    fn default() -> Self {
        NullKeyManager
    }
}

#[async_trait]
impl KeyManager for NullKeyManager {
    async fn watch(
        &self,
        _namespace: String,
        _deployment: String,
        _config_key: String,
        _consul_key: String,
    ) -> Result<bool> {
        Ok(true)
    }

    async fn unwatch_namespace(&self, _namespace: String) -> Result<usize> {
        Ok(0)
    }

    async fn unwatch_deployment(&self, _namespace: String, _deployment: String) -> Result<usize> {
        Ok(0)
    }

    async fn set(&self, _key: String, _value: String) -> Result<()> {
        Ok(())
    }

    async fn get(&self, _key: String) -> Result<Option<String>> {
        Ok(None)
    }

    async fn subscriptions_for_deployment(
        &self,
        _namespace: String,
        _deployment: String,
    ) -> Result<Vec<Subscription>> {
        Ok(vec![])
    }
}

#[derive(Default)]
struct InnerMemoryKeyManager {
    checksums: HashMap<String, String>,
    subscriptions: HashMap<Subscription, String>,
}

#[derive(Default)]
pub struct MemoryKeyManager {
    inner: Mutex<RefCell<InnerMemoryKeyManager>>,
}

#[async_trait]
impl KeyManager for MemoryKeyManager {
    async fn watch(
        &self,
        namespace: String,
        deployment: String,
        config_key: String,
        consul_key: String,
    ) -> Result<bool> {
        let inner_lock = self.inner.lock();
        let mut inner = inner_lock.borrow_mut();

        // Return an error if the subscription (resource in a namespace for a key) already exists.
        let subscription = Subscription {
            namespace,
            deployment,
            config_key,
        };
        if let Some(val) = inner.subscriptions.get(&subscription) {
            if val != &consul_key {
                return Err(anyhow!("subscription already exists"));
            }
            // Return false if the subscription already exists and points to the same consul key.
            return Ok(false);
        }

        inner.subscriptions.insert(subscription, consul_key);

        Ok(true)
    }

    async fn unwatch_namespace(&self, namespace: String) -> Result<usize> {
        let inner_lock = self.inner.lock();
        let mut inner = inner_lock.borrow_mut();

        let count = inner.subscriptions.len();
        inner.subscriptions.retain(|k, _| k.namespace != namespace);
        let modified_count = inner.subscriptions.len();

        Ok(count - modified_count)
    }

    async fn unwatch_deployment(&self, namespace: String, deployment: String) -> Result<usize> {
        let inner_lock = self.inner.lock();
        let mut inner = inner_lock.borrow_mut();

        let count = inner.subscriptions.len();
        inner
            .subscriptions
            .retain(|k, _| k.namespace != namespace && k.deployment != deployment);
        let modified_count = inner.subscriptions.len();

        Ok(count - modified_count)
    }

    async fn set(&self, key: String, value: String) -> Result<()> {
        let inner_lock = self.inner.lock();
        let mut inner = inner_lock.borrow_mut();

        inner.checksums.insert(key, value);

        Ok(())
    }

    async fn get(&self, state: String) -> Result<Option<String>> {
        let inner_lock = self.inner.lock();
        let inner = inner_lock.borrow();

        match inner.checksums.get(&state) {
            Some(val_ref) => Ok(Some(val_ref.to_owned())),
            None => Ok(None),
        }
    }

    async fn subscriptions_for_deployment(
        &self,
        namespace: String,
        deployment: String,
    ) -> Result<Vec<Subscription>> {
        let inner_lock = self.inner.lock();
        let inner = inner_lock.borrow_mut();

        let mut results = vec![];

        for subscription in inner.subscriptions.iter() {
            if subscription.0.namespace == namespace && subscription.0.deployment == deployment {
                results.push(subscription.0.clone());
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_manager::NullKeyManager;

    #[tokio::test]
    async fn null_key_manager() {
        let key_manager = Box::new(NullKeyManager::default()) as Box<dyn KeyManager>;
        let watch_res = key_manager
            .watch(
                "default".to_string(),
                "app-foo".to_string(),
                "config".to_string(),
                "config".to_string(),
            )
            .await;
        assert!(watch_res.is_ok());
        assert_eq!(watch_res.unwrap(), true);
    }

    #[tokio::test]
    async fn memory_key_manager() {
        let key_manager = Box::new(MemoryKeyManager::default()) as Box<dyn KeyManager>;
        {
            let watch_res = key_manager
                .watch(
                    "default".to_string(),
                    "app-foo".to_string(),
                    "config".to_string(),
                    "config".to_string(),
                )
                .await;
            assert!(watch_res.is_ok());
            assert_eq!(watch_res.unwrap(), true);
        }
        {
            let watch_res2 = key_manager
                .watch(
                    "default".to_string(),
                    "app-foo".to_string(),
                    "config".to_string(),
                    "config".to_string(),
                )
                .await;
            assert!(watch_res2.is_ok());
            assert_eq!(watch_res2.unwrap(), false);
        }
        {
            let watch_res3 = key_manager
                .watch(
                    "default".to_string(),
                    "app-foo".to_string(),
                    "config".to_string(),
                    "nah".to_string(),
                )
                .await;
            assert!(watch_res3.is_err());
        }
    }
}
