use super::tree::Tree;
use crate::author::Author;
use crate::error::StateError;

pub struct Acl {
    pub(crate) tree: sled::Tree,
}

impl Acl {
    pub fn from_tree(tree: sled::Tree) -> Self {
        Self { tree }
    }

    pub fn insert(&self, _author: Author, key: &[u8], value: &[u8]) -> Result<(), StateError> {
        self.tree.insert(key, value)?;
        Ok(())
    }

    pub fn remove(&self, _author: Author, key: &[u8]) -> Result<(), StateError> {
        self.tree.remove(key)?;
        Ok(())
    }

    pub fn compare_and_swap(
        &self,
        _author: Author,
        key: &[u8],
        old: Option<&[u8]>,
        new: Option<&[u8]>,
    ) -> Result<(), StateError> {
        self.tree.compare_and_swap(key, old, new)?.ok();
        Ok(())
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

    #[test]
    fn test_commit() {
        let id = Identity::generate();
        let tmpdir = TempDir::new("test_commit").unwrap();
        let path: &Path = tmpdir.path().into();
        let db = sled::open(path).unwrap();
        let tree = db.open_tree("state").unwrap();
        let state = Acl::from_tree(tree);

        state.insert(id.author(), b"key", b"value").unwrap();
        let tree = state.tree();
        let value = tree.get(b"key").unwrap();
        assert_eq!(value.as_ref().map(|v| v.as_ref()), Some(&b"value"[..]));
        state.remove(id.author(), b"key").unwrap();
        assert_eq!(tree.get(b"key").unwrap(), None);
    }
}
