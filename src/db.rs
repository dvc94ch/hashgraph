use crate::author::Author;
use crate::error::StateError;
use crate::state::State;
use async_std::path::Path;

pub struct Db {
    db: sled::Db,
    authors: sled::Tree,
    state: State,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, StateError> {
        let db = sled::open(path.join("sled"))?;
        let authors = db.open_tree("authors")?;
        let state = State::from_tree(db.open_tree("state")?);
        Ok(Self { db, authors, state })
    }

    pub fn authors(&self) -> Result<Vec<Author>, StateError> {
        self.authors
            .iter()
            .map(|res| Ok(Author::from_bytes(&res?.0)?))
            .collect()
    }
}

// Op::AddAuthor(author) => { self.authors.insert(author.as_bytes(), &[])?; }
// Op::RemAuthor(author) => { self.authors.remove(author.as_bytes())?; }
/*
    #[test]
    fn test_authors() {
        let id1 = Identity::generate();
        let id2 = Identity::generate();
        let tmpdir = TempDir::new("test_authors").unwrap();
        let path: &Path = tmpdir.path().into();
        let tree = sled::open(path).unwrap();
        let state = State::from_tree(tree).unwrap();
        assert_eq!(state.authors().unwrap(), vec![]);
        state.commit(Op::AddAuthor(id1.author())).unwrap();
        assert_eq!(state.authors().unwrap(), vec![id1.author()]);
        state.commit(Op::AddAuthor(id2.author())).unwrap();
        let mut authors = vec![id1.author(), id2.author()];
        authors.sort();
        assert_eq!(state.authors().unwrap(), authors);
        state.commit(Op::RemAuthor(id1.author())).unwrap();
        assert_eq!(state.authors().unwrap(), vec![id2.author()]);
    }
*/
