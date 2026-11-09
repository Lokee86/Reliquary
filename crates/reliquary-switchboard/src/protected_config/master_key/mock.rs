use super::{MasterKey, MasterKeyError, Zeroizing};
#[cfg(test)]
use super::{account, decode};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
#[cfg(test)]
use std::path::Path;
use std::rc::Rc;

type StoredKey = Zeroizing<Vec<u8>>;

#[derive(Default)]
struct Store {
    keys: RefCell<HashMap<String, StoredKey>>,
    fail_load: Cell<bool>,
    fail_write: Cell<bool>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Rc<Store>>> = const { RefCell::new(None) };
}

/// Installs an isolated, in-memory credential store for the current test thread.
#[must_use]
pub fn mock_master_key_store() -> MockMasterKeyStore {
    let store = Rc::new(Store::default());
    let previous = ACTIVE.replace(Some(store.clone()));
    MockMasterKeyStore { store, previous }
}

/// Restores the previous credential store when the test scope ends.
pub struct MockMasterKeyStore {
    store: Rc<Store>,
    previous: Option<Rc<Store>>,
}

impl Drop for MockMasterKeyStore {
    fn drop(&mut self) {
        ACTIVE.with_borrow_mut(|active| {
            if active
                .as_ref()
                .is_some_and(|store| Rc::ptr_eq(store, &self.store))
            {
                *active = self.previous.take();
            }
        });
    }
}

#[cfg(test)]
impl MockMasterKeyStore {
    fn account(path: &Path) -> String {
        account(&path.canonicalize().unwrap())
    }

    pub(crate) fn key(&self, path: &Path) -> Option<MasterKey> {
        decode(self.store.keys.borrow().get(&Self::account(path)).cloned()).unwrap()
    }

    pub(crate) fn replace(&self, path: &Path, bytes: Vec<u8>) {
        self.store
            .keys
            .borrow_mut()
            .insert(Self::account(path), Zeroizing::new(bytes));
    }

    pub(crate) fn remove(&self, path: &Path) {
        self.store.keys.borrow_mut().remove(&Self::account(path));
    }

    pub(crate) fn fail_load(&self, fail: bool) {
        self.store.fail_load.set(fail);
    }

    pub(crate) fn fail_write(&self, fail: bool) {
        self.store.fail_write.set(fail);
    }
}

pub(super) fn load(account: &str) -> Option<Result<Option<StoredKey>, MasterKeyError>> {
    ACTIVE.with_borrow(|active| {
        active.as_ref().map(|store| {
            if store.fail_load.get() {
                Err(MasterKeyError::StoreUnavailable)
            } else {
                Ok(store.keys.borrow().get(account).cloned())
            }
        })
    })
}

pub(super) fn write(account: &str, key: &MasterKey) -> Option<Result<(), MasterKeyError>> {
    ACTIVE.with_borrow(|active| {
        active.as_ref().map(|store| {
            if store.fail_write.get() {
                return Err(MasterKeyError::WriteRejected);
            }
            store
                .keys
                .borrow_mut()
                .insert(account.to_owned(), Zeroizing::new(key.to_vec()));
            Ok(())
        })
    })
}
