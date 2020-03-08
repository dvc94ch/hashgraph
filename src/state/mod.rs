mod chain;
mod checkpoint;
mod queue;
mod state_machine;
mod transaction;
mod tree;

use crate::author::{Author, Identity, Signature};
use crate::error::Error;
use crate::hash::{FileHasher, Hash};
use async_std::path::Path;
use chain::AuthorChain;
use checkpoint::ProposedCheckpoint;
pub use checkpoint::{Checkpoint, SignedCheckpoint};
use queue::TransactionQueue;
use state_machine::StateMachine;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
pub use transaction::*;
pub use tree::{Exporter, Importer, Tree};

pub struct State {
    db: sled::Db,
    authors: sled::Tree,
    state: sled::Tree,
    chain: AuthorChain,
    state_machine: StateMachine,
    queue: Arc<Mutex<TransactionQueue>>,
    checkpoint: Option<SignedCheckpoint>,
    proposed: Option<ProposedCheckpoint>,
}

impl State {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let db = sled::open(path.join("sled"))?;
        let authors = db.open_tree("authors")?;
        let state = db.open_tree("state")?;
        let chain = AuthorChain::from_tree(authors.clone())?;
        let state_machine = StateMachine::from_tree(state.clone());
        Ok(Self {
            db,
            authors,
            state,
            chain,
            state_machine,
            queue: Default::default(),
            checkpoint: None,
            proposed: None,
        })
    }

    pub fn genesis(&mut self, genesis_authors: HashSet<Author>) -> Result<(), Error> {
        self.chain.genesis(genesis_authors)
    }

    pub fn genesis_hash(&self) -> Result<Hash, Error> {
        self.chain.genesis_hash()
    }

    pub fn tree(&self) -> Tree {
        Tree::new(self.state.clone(), self.queue.clone())
    }

    pub fn create_payload(&self) -> Box<[Transaction]> {
        self.queue.lock().unwrap().create_payload()
    }

    pub fn commit(&mut self, author: &Author, tx: &Transaction) -> Result<(), Error> {
        let result = match tx {
            Transaction::AddAuthor(author, block) => Ok(self.chain.add_author(*author, *block)),
            Transaction::RemAuthor(author, block) => Ok(self.chain.rem_author(*author, *block)),
            Transaction::SignBlock(signature) => Ok(self.chain.sign_block(*author, *signature)),
            Transaction::Insert(key, value) => self.state_machine.insert(author, key, value)?,
            Transaction::Remove(key) => self.state_machine.remove(author, key)?,
            Transaction::CompareAndSwap(key, old, new) => {
                self.state_machine
                    .compare_and_swap(author, key, old.as_ref(), new.as_ref())?
            }
            Transaction::AddAuthorToPrefix(prefix, new) => self
                .state_machine
                .add_author_to_prefix(author, prefix.as_ref(), *new)?,
            Transaction::RemAuthorFromPrefix(prefix, rm) => self
                .state_machine
                .remove_author_from_prefix(author, prefix.as_ref(), *rm)?,
            Transaction::SignCheckpoint(signature) => Ok(self.sign_checkpoint(*author, *signature)),
        };
        self.queue.lock().unwrap().commit(tx, result)?;
        Ok(())
    }

    pub fn start_round(&mut self) -> Result<(u64, Box<[Author]>), Error> {
        self.chain.start_round()
    }

    pub fn sign_block(&self, identity: &Identity) -> Transaction {
        let block_hash = self.chain.hash().expect("proposed block exists");
        let signature = identity.sign(&*block_hash);
        Transaction::SignBlock(signature)
    }

    pub async fn export_checkpoint(&mut self, dir: &Path) -> Result<Checkpoint, Error> {
        let mut fh = FileHasher::create_tmp(&dir).await?;
        Exporter::new(&self.authors, &mut fh).write_tree().await?;
        Exporter::new(&self.state, &mut fh).write_tree().await?;
        let checkpoint = Checkpoint(fh.rename(&dir).await?);
        self.proposed = Some(ProposedCheckpoint::new(checkpoint));
        Ok(checkpoint)
    }

    pub async fn import_checkpoint(
        &mut self,
        dir: &Path,
        checkpoint: SignedCheckpoint,
    ) -> Result<(), Error> {
        let genesis = self.genesis_hash().ok();

        self.authors.clear()?;
        self.state.clear()?;
        let mut fh = FileHasher::open_with_hash(dir, &*checkpoint).await?;
        Importer::new(&self.authors, &mut fh).read_tree().await?;
        Importer::new(&self.state, &mut fh).read_tree().await?;
        if fh.hash() != *checkpoint {
            self.authors.clear()?;
            self.state.clear()?;
            return Err(Error::InvalidCheckpoint);
        }

        // make sure that it's still the same chain by comparing the new genesis hash.
        let chain = AuthorChain::from_tree(self.authors.clone())?;
        if let Some(genesis) = genesis {
            let new_genesis = chain.genesis_hash()?;
            if genesis != new_genesis {
                return Err(Error::InvalidCheckpoint);
            }
        }

        // check the signatures
        let population = chain.authors.len();
        let threshold = population - population * 2 / 3;
        let mut signees = HashSet::new();
        for sig in &checkpoint.signatures[..] {
            for author in chain.authors.iter() {
                if signees.contains(author) {
                    continue;
                }
                if author.verify(&**checkpoint, sig).is_err() {
                    continue;
                }
                signees.insert(*author);
            }
        }
        if signees.len() < threshold {
            return Err(Error::InvalidCheckpoint);
        }

        self.chain = chain;
        self.checkpoint = Some(checkpoint);
        Ok(())
    }

    pub fn checkpoint(&self) -> Option<&SignedCheckpoint> {
        self.checkpoint.as_ref()
    }

    fn sign_checkpoint(&mut self, author: Author, sig: Signature) {
        if let Some(mut proposed) = self.proposed.take() {
            proposed.add_sig(author, sig);
            let population = self.chain.authors.len();
            let threshold = population - population * 2 / 3;
            if proposed.len() >= threshold {
                self.checkpoint = Some(proposed.into_signed_checkpoint());
            } else {
                self.proposed = Some(proposed);
            }
        }
    }

    pub fn flush(&self) -> Result<(), Error> {
        self.db.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::Identity;
    use tempdir::TempDir;

    fn gen_ids(n: usize) -> Vec<Identity> {
        let mut ids = Vec::with_capacity(n);
        for _ in 0..n {
            ids.push(Identity::generate());
        }
        ids
    }

    fn set(ids: &[Identity]) -> HashSet<Author> {
        let mut set = HashSet::new();
        for id in ids {
            set.insert(id.author());
        }
        set
    }

    #[async_std::test]
    async fn test_insert() {
        let ids = gen_ids(1);
        let tmpdir = TempDir::new("test_insert").unwrap();
        let path: &Path = tmpdir.path().into();
        let mut state = State::open(path).unwrap();
        state.genesis(set(&ids)).unwrap();
        let tree = state.tree();
        let fut = tree.insert(b"prefix", b"key", Value::new("value")).unwrap();
        let txs = state.create_payload();
        for tx in txs.iter() {
            println!("{:?}", tx);
            state.commit(&ids[0].author(), &tx).unwrap();
        }
        let value = tree.get(Key::new(b"prefix", b"key").unwrap()).unwrap();
        assert_eq!(value.as_ref().map(|v| v.as_ref()), Some(&b"value"[..]));
        assert!(fut.await.is_ok());
    }

    #[test]
    fn test_authors() {
        let ids = gen_ids(4);
        let tmpdir = TempDir::new("test_authors").unwrap();
        let path: &Path = tmpdir.path().into();
        let mut state = State::open(path).unwrap();
        state.genesis(set(&ids[..2])).unwrap();

        let (block, authors) = state.start_round().unwrap();
        assert_eq!(block, 1);
        assert_eq!(authors.len(), 2);
        state
            .commit(
                &ids[0].author(),
                &Transaction::AddAuthor(ids[2].author(), 1),
            )
            .unwrap();
        state
            .commit(
                &ids[0].author(),
                &Transaction::RemAuthor(ids[0].author(), 1),
            )
            .unwrap();

        let (block2, authors2) = state.start_round().unwrap();
        assert_eq!(block2, 1);
        assert_eq!(authors, authors2);
        state
            .commit(&ids[0].author(), &state.sign_block(&ids[0]))
            .unwrap();

        let (block3, authors3) = state.start_round().unwrap();
        assert_eq!(block3, 2);
        assert_eq!(authors3.len(), 2);
        assert_ne!(authors3, authors);
    }

    #[async_std::test]
    async fn test_export_import() {
        let ids = gen_ids(2);
        let tmpdir = TempDir::new("test_export_import").unwrap();
        let path: &Path = tmpdir.path().into();
        let mut state = State::open(path).unwrap();
        state.genesis(set(&ids)).unwrap();

        let dir = path.join("checkpoint");
        async_std::fs::create_dir_all(&dir).await.unwrap();

        let key = Key::new(b"prefix", b"key").unwrap();
        let value = Value::new(b"value");
        let tx = Transaction::Insert(key.clone(), value.clone());
        state.commit(&ids[0].author(), &tx).unwrap();

        let checkpoint = state.export_checkpoint(&dir).await.unwrap();

        let signed = SignedCheckpoint {
            checkpoint,
            signatures: vec![ids[0].sign(&**checkpoint)].into_boxed_slice(),
        };
        state.import_checkpoint(&dir, signed).await.unwrap();

        let checkpoint2 = state.export_checkpoint(&dir).await.unwrap();
        assert_eq!(checkpoint, checkpoint2);
    }
}
