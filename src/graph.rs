//! Gossip graph
use crate::event::{DerivedProperties, Event, RawEvent, RawProperties};
use multihash::Multihash;
use std::collections::HashMap;

/// Hash graph.
#[derive(Default)]
pub struct HashGraph {
    events: HashMap<Multihash, Event>,
    population: u32,
}

// Parents and ancestors
impl HashGraph {
    /// Set of parents of an event.
    pub fn parents<TEvent: RawProperties>(&self, event: &TEvent) -> Vec<&Event> {
        event
            .parent_hashes()
            .into_iter()
            .filter_map(|mh| self.events.get(mh))
            .collect()
    }

    /// Self parent of an event.
    pub fn self_parent<TEvent: RawProperties>(&self, event: &TEvent) -> Option<&Event> {
        event
            .self_parent_hash()
            .map(|mh| self.events.get(mh))
            .unwrap_or_default()
    }

    /// Returns an iterator of an events ancestors.
    pub fn ancestors<'a, TEvent>(&'a self, event: &'a TEvent) -> AncestorIter<'a, TEvent>
    where
        TEvent: RawProperties,
        &'a TEvent: From<&'a Event>,
    {
        AncestorIter {
            graph: self,
            stack: vec![Box::new(vec![event].into_iter())],
        }
    }

    /// Returns an iterator of an events self ancestors.
    pub fn self_ancestors<'a, TEvent>(&'a self, event: &'a TEvent) -> SelfAncestorIter<'a, TEvent>
    where
        TEvent: RawProperties,
        &'a TEvent: From<&'a Event>,
    {
        SelfAncestorIter {
            graph: self,
            event: Some(event),
        }
    }

    /// Event x is an ancestor of y if x can reach y by following 0 or more
    /// parent edges.
    pub fn ancestor<'a, TEvent>(&'a self, x: &'a TEvent, y: &TEvent) -> bool
    where
        TEvent: RawProperties + 'a,
        &'a TEvent: From<&'a Event>,
    {
        self.ancestors(x).find(|e| e.hash() == y.hash()).is_some()
    }

    /// Event x is a self_ancestor of y if x can reach y by following 0 or more
    /// self_parent edges.
    pub fn self_ancestor<'a, TEvent>(&'a self, x: &'a TEvent, y: &TEvent) -> bool
    where
        TEvent: RawProperties + 'a,
        &'a TEvent: From<&'a Event>,
    {
        self.self_ancestors(x)
            .find(|e| e.hash() == y.hash())
            .is_some()
    }
}

/// Iterator of ancestors.
pub struct AncestorIter<'a, TEvent> {
    graph: &'a HashGraph,
    stack: Vec<Box<dyn Iterator<Item = &'a TEvent> + 'a>>,
}

impl<'a, TEvent> Iterator for AncestorIter<'a, TEvent>
where
    TEvent: RawProperties,
    &'a TEvent: From<&'a Event>,
{
    type Item = &'a TEvent;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(iter) = self.stack.last_mut() {
                if let Some(event) = iter.next() {
                    let parents = self.graph.parents(event);
                    self.stack
                        .push(Box::new(parents.into_iter().map(Into::into)));
                    return Some(event);
                } else {
                    self.stack.pop();
                }
            } else {
                return None;
            }
        }
    }
}

/// Iterator of self ancestors.
pub struct SelfAncestorIter<'a, TEvent> {
    graph: &'a HashGraph,
    event: Option<&'a TEvent>,
}

impl<'a, TEvent> Iterator for SelfAncestorIter<'a, TEvent>
where
    TEvent: RawProperties,
    &'a TEvent: From<&'a Event>,
{
    type Item = &'a TEvent;

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
impl HashGraph {
    /// Event x sees y if y is an ancestor of x, but no fork of y is an
    /// ancestor of x.
    pub fn see<'a, TEvent>(&'a self, x: &'a TEvent, y: &TEvent) -> bool
    where
        TEvent: RawProperties + 'a,
        &'a TEvent: From<&'a Event>,
    {
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

    /// Event x strongly sees y if x can see events by more than 2n/3 creators,
    /// each of which sees y.
    pub fn strongly_see<'a, TEvent>(&'a self, x: &'a TEvent, y: &'a TEvent) -> bool
    where
        TEvent: DerivedProperties + 'a,
        &'a TEvent: From<&'a Event>,
    {
        let ay: Vec<u32> = (0..self.population)
            .into_iter()
            .map(|n| {
                self.ancestors(y)
                    .find(|ancestor| ancestor.author() == n)
                    .map(|ancestor| ancestor.seq())
                    .unwrap_or(1)
            })
            .collect();
        let ax: Vec<u32> = (0..self.population)
            .into_iter()
            .map(|n| {
                self.ancestors(x)
                    // TODO more efficient traversal
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                    .find(|ancestor| ancestor.author() == n)
                    .map(|ancestor| ancestor.seq())
                    .unwrap_or(1)
            })
            .collect();
        let number_of_authors_see = ay.into_iter().zip(ax).filter(|(y, x)| y >= x).count();
        let threshold = 2 * self.population as usize / 3;
        number_of_authors_see > threshold
    }
}

impl HashGraph {
    pub fn end_of_round(&self, witnesses: Vec<Event>, raw: &RawEvent) -> bool {
        let threshold = 2 * self.population as usize / 3;
        let mut count = 0;
        for witness in &witnesses {
            if self.strongly_see(raw, witness) {
                count += 1;
                if count > threshold {
                    return true;
                }
            }
        }
        false
    }

    /// Adds a raw event to the graph.
    ///
    /// The maximum created round of all self parents of x (or 1 if there are none).
    /// Event x is a witness if x has a greater created round than its self parent.
    pub fn add_event<'a>(&'a mut self, raw: RawEvent) -> &'a Event {
        let seq = if let Some(parent) = self.self_parent(&raw) {
            parent.seq() + 1
        } else {
            1
        };
        let parent_round = self
            .parents(&raw)
            .into_iter()
            .map(|p| p.round())
            .max()
            .unwrap_or(1);
        // TODO get real witnesses for parent_round
        let round = if self.end_of_round(vec![], &raw) {
            parent_round + 1
        } else {
            parent_round
        };
        let witness = round > parent_round;
        let event = Event {
            raw,
            seq,
            round,
            witness,
        };
        let hash = event.hash().clone();
        self.events.insert(hash.clone(), event);
        self.events.get(&hash).expect("just inserted; qed")
    }
}
