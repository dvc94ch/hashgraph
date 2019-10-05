//! Gossip graph
use crate::event::RawProperties;
use multihash::Multihash;
use std::collections::HashMap;

/// Gossip graph.
pub struct Graph<TGraphEvent> {
    events: HashMap<Multihash, TGraphEvent>,
}

// Parents and ancestors
impl<TGraphEvent: RawProperties> Graph<TGraphEvent> {
    /// Get the event.
    pub fn get(&self, hash: &Multihash) -> Option<&TGraphEvent> {
        self.events.get(hash)
    }

    /// Set of parents of an event.
    pub fn parents<TEvent: RawProperties>(&self, event: &TEvent) -> Vec<&TGraphEvent> {
        event
            .parent_hashes()
            .into_iter()
            .filter_map(|mh| self.events.get(mh))
            .collect()
    }

    /// Self parent of an event.
    pub fn self_parent<TEvent: RawProperties>(&self, event: &TEvent) -> Option<&TGraphEvent> {
        event
            .self_parent_hash()
            .map(|mh| self.events.get(mh))
            .unwrap_or_default()
    }

    /// Returns an iterator of an events ancestors.
    pub fn ancestors<'a, TEvent>(
        &'a self,
        event: &'a TEvent,
    ) -> AncestorIter<'a, TGraphEvent, TEvent>
    where
        TEvent: RawProperties,
        &'a TEvent: From<&'a TGraphEvent>,
    {
        AncestorIter {
            graph: self,
            stack: vec![Box::new(vec![event].into_iter())],
        }
    }

    /// Returns an iterator of an events self ancestors.
    pub fn self_ancestors<'a, TEvent>(
        &'a self,
        event: &'a TEvent,
    ) -> SelfAncestorIter<'a, TGraphEvent, TEvent>
    where
        TEvent: RawProperties + 'a,
        &'a TEvent: From<&'a TGraphEvent>,
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
        &'a TEvent: From<&'a TGraphEvent>,
    {
        self.ancestors(x).find(|e| e.hash() == y.hash()).is_some()
    }

    /// Event x is a self_ancestor of y if x can reach y by following 0 or more
    /// self_parent edges.
    pub fn self_ancestor<'a, TEvent>(&'a self, x: &'a TEvent, y: &TEvent) -> bool
    where
        TEvent: RawProperties + 'a,
        &'a TEvent: From<&'a TGraphEvent>,
    {
        self.self_ancestors(x)
            .find(|e| e.hash() == y.hash())
            .is_some()
    }
}

/// Iterator of ancestors.
pub struct AncestorIter<'a, TGraphEvent, TEvent> {
    graph: &'a Graph<TGraphEvent>,
    stack: Vec<Box<dyn Iterator<Item = &'a TEvent> + 'a>>,
}

impl<'a, TGraphEvent, TEvent> Iterator for AncestorIter<'a, TGraphEvent, TEvent>
where
    TGraphEvent: RawProperties,
    TEvent: RawProperties,
    &'a TEvent: From<&'a TGraphEvent>,
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
pub struct SelfAncestorIter<'a, TGraphEvent, TEvent> {
    graph: &'a Graph<TGraphEvent>,
    event: Option<&'a TEvent>,
}

impl<'a, TGraphEvent, TEvent> Iterator for SelfAncestorIter<'a, TGraphEvent, TEvent>
where
    TGraphEvent: RawProperties,
    TEvent: RawProperties,
    &'a TEvent: From<&'a TGraphEvent>,
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
impl<TGraphEvent: RawProperties> Graph<TGraphEvent> {
    /// Event x sees y if y is an ancestor of x, but no fork of y is an
    /// ancestor of x.
    pub fn see<'a, TEvent>(&'a self, x: &'a TEvent, y: &TEvent) -> bool
    where
        TEvent: RawProperties + 'a,
        &'a TEvent: From<&'a TGraphEvent>,
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

    /// Event x strongly sees y if x can see events by more than 2n/3 authors,
    /// each of which sees y.
    pub fn strongly_see<'a, TEvent>(&'a self, x: &'a TEvent, y: &'a TEvent, n: u32) -> bool
    where
        TEvent: RawProperties + 'a,
        &'a TEvent: From<&'a TGraphEvent>,
    {
        let ay: Vec<u32> = (0..n)
            .into_iter()
            .map(|n| {
                self.ancestors(y)
                    .find(|ancestor| ancestor.author() == n)
                    .map(|ancestor| ancestor.seq())
                    .unwrap_or(1)
            })
            .collect();
        let ax: Vec<u32> = (0..n)
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
        number_of_authors_see > 2 * n as usize / 3
    }
}

impl<TEvent: RawProperties> Graph<TEvent> {
    /// Adds an event to the graph.
    pub fn add_event(&mut self, event: TEvent) {
        self.events.insert(event.hash().clone(), event);
    }

    /// Removes an event from the graph.
    pub fn remove_event(&mut self, event: TEvent) {
        let ancestors: Vec<_> = self.ancestors(&event).map(|e| e.hash().clone()).collect();
        for ancestor in ancestors {
            self.events.remove(&ancestor);
        }
    }
}
