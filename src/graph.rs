//! Gossip graph
use crate::author::Author;
use crate::error::Error;
use crate::event::{Event, RawEvent};
use crate::hash::Hash;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Gossip graph.
#[derive(Debug)]
pub struct Graph<T> {
    state: HashMap<Author, u64>,
    events: HashMap<Hash, Event<T>>,
    root: Option<Hash>,
}

impl<T> Default for Graph<T> {
    fn default() -> Self {
        Self {
            state: Default::default(),
            events: Default::default(),
            root: Default::default(),
        }
    }
}

// Parents and ancestors
impl<T> Graph<T> {
    /// Set of parents of an event.
    pub fn parents(&self, event: &Event<T>) -> Vec<&Event<T>> {
        event
            .parent_hashes()
            .into_iter()
            .filter_map(|mh| self.events.get(mh))
            .collect()
    }

    /// Self parent of an event.
    pub fn self_parent(&self, event: &Event<T>) -> Option<&Event<T>> {
        event
            .self_parent_hash()
            .map(|mh| self.events.get(mh))
            .unwrap_or_default()
    }

    /// Returns an iterator of an events ancestors.
    pub fn ancestors<'a>(&'a self, event: &'a Event<T>) -> AncestorIter<'a, T> {
        AncestorIter {
            graph: self,
            stack: vec![event],
            visited: HashSet::new(),
        }
    }

    /// Returns an iterator of an events self ancestors.
    pub fn self_ancestors<'a>(&'a self, event: &'a Event<T>) -> SelfAncestorIter<'a, T> {
        SelfAncestorIter {
            graph: self,
            event: Some(event),
        }
    }

    /// Event x is an ancestor of y if x can reach y by following 0 or more
    /// parent edges.
    pub fn ancestor<'a>(&'a self, x: &'a Event<T>, y: &Event<T>) -> bool {
        self.ancestors(x).find(|e| e.hash() == y.hash()).is_some()
    }

    /// Event x is a self_ancestor of y if x can reach y by following 0 or more
    /// self_parent edges.
    pub fn self_ancestor<'a>(&'a self, x: &'a Event<T>, y: &Event<T>) -> bool {
        self.self_ancestors(x)
            .find(|e| e.hash() == y.hash())
            .is_some()
    }
}

/// Iterator of ancestors.
pub struct AncestorIter<'a, T> {
    graph: &'a Graph<T>,
    stack: Vec<&'a Event<T>>,
    visited: HashSet<Hash>,
}

impl<'a, T> Iterator for AncestorIter<'a, T> {
    type Item = &'a Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.stack.pop() {
            self.visited.insert(*event.hash());
            for parent in self.graph.parents(event) {
                if !self.visited.contains(parent.hash()) {
                    self.stack.push(parent);
                }
            }
            Some(event)
        } else {
            None
        }
    }
}

/// Iterator of self ancestors.
pub struct SelfAncestorIter<'a, T> {
    graph: &'a Graph<T>,
    event: Option<&'a Event<T>>,
}

impl<'a, T> Iterator for SelfAncestorIter<'a, T> {
    type Item = &'a Event<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_event = if let Some(event) = self.event.as_ref() {
            self.graph.self_parent(*event).map(Into::into)
        } else {
            None
        };
        let event = self.event.take();
        self.event = next_event;
        event
    }
}

// seeing
impl<T> Graph<T> {
    /// Event x sees y if y is an ancestor of x, but no fork of y is an
    /// ancestor of x.
    pub fn see(&self, x: &Hash, y: &Hash) -> bool {
        let (x, y) = (self.event(x).unwrap(), self.event(y).unwrap());
        let mut is_ancestor = false;
        let mut created = Vec::new();
        for ancestor in self.ancestors(x) {
            if !is_ancestor && ancestor.hash() == y.hash() {
                is_ancestor = true;
            }
            if ancestor.author() == y.author() {
                created.push(ancestor);
            }
        }
        if !is_ancestor {
            return false;
        }
        for (i, a) in created.iter().enumerate() {
            for b in created[(i + 1)..].iter() {
                if !self.self_ancestor(a, b) && !self.self_ancestor(b, a) {
                    return false;
                }
            }
        }
        true
    }

    /// Event x strongly sees y if x can see events by more than 2n/3 authors,
    /// each of which sees y.
    pub fn strongly_see(&self, x: &Hash, y: &Hash, authors: &[Author]) -> bool {
        let (x, y) = (self.event(x).unwrap(), self.event(y).unwrap());
        let ay: Vec<_> = authors
            .iter()
            .map(|author| {
                self.ancestors(y)
                    .find(|ancestor| ancestor.author() == *author)
                    .map(|ancestor| ancestor.seq())
                    .unwrap_or(1)
            })
            .collect();
        let ax: Vec<_> = authors
            .iter()
            .map(|author| {
                self.ancestors(x)
                    // TODO more efficient traversal
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                    .find(|ancestor| ancestor.author() == *author)
                    .map(|ancestor| ancestor.seq())
                    .unwrap_or(1)
            })
            .collect();
        let number_of_authors_see = ay.into_iter().zip(ax).filter(|(y, x)| y >= x).count();
        number_of_authors_see > 2 * authors.len() / 3
    }
}

impl<T> Graph<T> {
    /// Retrieves an event from the graph.
    pub fn event(&self, hash: &Hash) -> Option<&Event<T>> {
        self.events.get(hash)
    }

    /// Retrieves a mutable event from the graph.
    pub fn event_mut(&mut self, hash: &Hash) -> Option<&mut Event<T>> {
        self.events.get_mut(hash)
    }

    /// Removes an event from the graph.
    pub fn remove_event(&mut self, event: Event<T>) {
        let ancestors: Vec<_> = self.ancestors(&event).map(|e| e.hash().clone()).collect();
        for ancestor in ancestors {
            self.events.remove(&ancestor);
        }
    }
}

impl<T: Serialize> Graph<T> {
    /// Adds an event to the graph.
    pub fn add_event(&mut self, event: RawEvent<T>) -> Result<Hash, Error> {
        let seq = if let Some(parent) = &event.event.self_hash {
            self.events.get(parent).ok_or(Error::InvalidEvent)?.seq() + 1
        } else {
            1
        };
        if let Some(parent) = &event.event.other_hash {
            self.events.get(parent).ok_or(Error::InvalidEvent)?;
        }
        let author = event.event.author;
        let hash = event.event.hash()?;
        author.verify(&*hash, &event.signature)?;
        let event = Event::new(event, hash, seq);
        self.events.insert(hash, event);
        self.state.insert(author, seq);
        self.root = Some(hash);
        Ok(hash)
    }
}

impl<T> Graph<T> {
    pub fn sync_state(&self, authors: &[Author]) -> Box<[Option<u64>]> {
        authors
            .iter()
            .map(|author| self.state.get(author).cloned())
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    pub fn sync<'a>(
        &self,
        state: HashMap<Author, u64>,
    ) -> impl Iterator<Item = &RawEvent<T>> {
        let mut stack = vec![];
        let mut gray = vec![];
        let mut black = HashSet::new();
        let mut post_order = vec![];
        if let Some(root) = self.root.as_ref() {
            stack.push(self.event(root).unwrap());
        }
        while let Some(event) = stack.pop() {
            if black.contains(&event.hash()) {
                continue;
            }
            if event.seq() <= state.get(&event.author()).cloned().unwrap_or(0) {
                black.insert(event.hash());
                continue;
            }
            for parent in self.parents(event) {
                if !black.contains(&parent.hash()) {
                    gray.push(parent);
                }
            }
            if gray.is_empty() {
                black.insert(event.hash());
                post_order.push(event);
            } else {
                stack.push(event);
                for e in gray.drain(..) {
                    stack.push(e);
                }
            }
        }
        post_order.into_iter().map(|e| &e.raw)
    }

    pub fn display(&self, authors: &[Author]) {
        let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let names: HashMap<_, _> = authors.into_iter().zip(alphabet.chars()).collect();
        if let Some(root) = &self.root {
            let event = self.event(root).unwrap();
            for event in self.ancestors(event) {
                let other_parent = event
                    .raw
                    .event
                    .other_hash
                    .as_ref()
                    .map(|hash| self.event(hash).unwrap());
                let name = names.get(&event.author()).unwrap();
                if let Some(other_parent) = other_parent {
                    let other_name = names.get(&other_parent.author()).unwrap();
                    println!(
                        "{}.{} -> {}.{}",
                        name,
                        event.seq(),
                        other_name,
                        other_parent.seq(),
                    );
                } else {
                    println!("{}.{} -> None", name, event.seq(),);
                }
            }
        }
    }
}
