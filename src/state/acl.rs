use super::key::{Key, Value};
use super::transaction::{TransactionError, TransactionResult};
use super::tree::Tree;
use crate::author::Author;
use crate::error::Error;
use sled::CompareAndSwapError;

pub struct Acl {
    pub(crate) tree: sled::Tree,
}

impl Acl {
    pub fn from_tree(tree: sled::Tree) -> Self {
        Self { tree }
    }

    pub fn add_author_to_prefix(
        &self,
        author: &Author,
        prefix: &[u8],
        new: Author,
    ) -> Result<TransactionResult, Error> {
        let mut authors = if let Some(value) = self.tree.get(&prefix)? {
            let authors: Vec<Author> = bincode::deserialize(&value)?;
            authors
        } else {
            Default::default()
        };
        if !authors.is_empty() {
            let mut has_permission = false;
            let mut contains_author = false;
            for a in &authors {
                if a == author {
                    has_permission = true;
                }
                if a == &new {
                    contains_author = true;
                }
            }
            if !has_permission {
                return Ok(Err(TransactionError::Permission));
            }
            if contains_author {
                return Ok(Ok(()));
            }
        }
        authors.push(new);
        self.tree.insert(prefix, bincode::serialize(&authors)?)?;
        Ok(Ok(()))
    }

    pub fn remove_author_from_prefix(
        &self,
        author: &Author,
        prefix: &[u8],
        rm: Author,
    ) -> Result<TransactionResult, Error> {
        let authors = if let Some(value) = self.tree.get(&prefix)? {
            let authors: Vec<Author> = bincode::deserialize(&value)?;
            authors
        } else {
            Default::default()
        };
        let mut new_authors = Vec::with_capacity(authors.len());
        let mut has_permission = false;
        let mut contains_author = false;
        for a in &authors {
            if a == author {
                has_permission = true;
            }
            if a == &rm {
                contains_author = true;
            } else {
                new_authors.push(a);
            }
        }
        if !has_permission {
            return Ok(Err(TransactionError::Permission));
        }
        if !contains_author {
            return Ok(Ok(()));
        }
        if new_authors.is_empty() {
            self.tree.remove(prefix)?;
        } else {
            self.tree
                .insert(prefix, bincode::serialize(&new_authors)?)?;
        }
        Ok(Ok(()))
    }

    pub fn insert(
        &self,
        author: &Author,
        key: &Key,
        value: &Value,
    ) -> Result<TransactionResult, Error> {
        match self.add_author_to_prefix(author, key.prefix(), *author)? {
            Ok(()) => {
                self.tree.insert(&key, value.as_ref())?;
                Ok(Ok(()))
            }
            Err(err) => Ok(Err(err)),
        }
    }

    pub fn remove(&self, author: &Author, key: &Key) -> Result<TransactionResult, Error> {
        match self.add_author_to_prefix(author, key.prefix(), *author)? {
            Ok(()) => {
                self.tree.remove(&key)?;
                Ok(Ok(()))
            }
            Err(err) => Ok(Err(err)),
        }
    }

    pub fn compare_and_swap(
        &self,
        author: &Author,
        key: &Key,
        old: Option<&Value>,
        new: Option<&Value>,
    ) -> Result<TransactionResult, Error> {
        match self.add_author_to_prefix(author, key.prefix(), *author)? {
            Ok(()) => {
                match self.tree.compare_and_swap(
                    key,
                    old.map(|v| v.as_ref()),
                    new.map(|v| v.as_ref()),
                )? {
                    Ok(()) => Ok(Ok(())),
                    Err(CompareAndSwapError { current, proposed }) => {
                        Ok(Err(TransactionError::CompareAndSwap {
                            current: current.map(Value::new),
                            proposed: proposed.map(Value::new),
                        }))
                    }
                }
            }
            Err(err) => Ok(Err(err)),
        }
    }

    pub fn tree(&self) -> Tree {
        Tree(self.tree.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::Identity;
    use async_std::path::Path;
    use tempdir::TempDir;

    fn setup() -> (TempDir, Acl, Tree) {
        let tmpdir = TempDir::new("test_commit").unwrap();
        let path: &Path = tmpdir.path().into();
        let db = sled::open(path).unwrap();
        let tree = db.open_tree("state").unwrap();
        let state = Acl::from_tree(tree);
        let tree = state.tree();
        (tmpdir, state, tree)
    }

    #[test]
    fn test_commit() {
        let id = Identity::generate();
        let (_, state, tree) = setup();
        let key = Key::new(b"prefix", b"key").unwrap();
        let value = Value::new(b"value");
        state.insert(&id.author(), &key, &value).unwrap().unwrap();
        let value = tree.get(&key).unwrap();
        assert_eq!(value.as_ref().map(|v| v.as_ref()), Some(&b"value"[..]));
        state.remove(&id.author(), &key).unwrap().unwrap();
        assert_eq!(tree.get(&key).unwrap(), None);
    }

    #[test]
    fn test_permission() {
        let id1 = Identity::generate();
        let id2 = Identity::generate();
        let (_, state, tree) = setup();
        let key = Key::new(b"prefix", b"key").unwrap();
        let v1 = Value::new(0u64.to_be_bytes());
        let v2 = Value::new(1u64.to_be_bytes());

        state.insert(&id1.author(), &key, &v1).unwrap().unwrap();
        let res = state.insert(&id2.author(), &key, &v2).unwrap();
        assert_eq!(res, Err(TransactionError::Permission));
        let value = tree.get(&key).unwrap();
        assert_eq!(value.as_ref().map(|v| v.as_ref()), Some(v1.as_ref()));

        state
            .add_author_to_prefix(&id1.author(), b"prefix", id2.author())
            .unwrap()
            .unwrap();
        state.insert(&id2.author(), &key, &v2).unwrap().unwrap();
        let value = tree.get(&key).unwrap();
        assert_eq!(value.as_ref().map(|v| v.as_ref()), Some(v2.as_ref()));
    }
}
