
// voting
/*
    /// round(x) - round(y)
    pub fn diff(&self, x: &Event, y: &Event) -> i32 {
        (self.round(x) as i64 - self.round(y) as i64) as i32
    }

    /// The number of votes equal to v about the fame of witness y collected by
    /// witness x from witnesses in the previous round.
    pub fn votes(&self, x: &Event, y: &Event, v: bool) -> u32 {
        0
    }

    /// Fraction of votes equal to true, regarding the fame of witness y,
    /// collected by witness x from witnesses in the previous round.
    pub fn fract_true(&self, x: &Event, y: &Event) -> u32 {
        0
    }

    /// If x (or its self ancestor) "decided" for the election for witness y
    /// (and therefore that member will never change its vote about y again).
    pub fn decide(&self, x: &Event, y: &Event) -> bool {
        false
    }

    /// If x should simply copy its selfParent's vote about the fame of witness
    /// y (or x is not a witness, or has already decided earlier).
    pub fn copy_vote(&self, x: &Event, y: &Event) -> bool {
        false
    }

    /// The vote by witness x about the fame of witness y (true for famous,
    /// false for not).
    pub fn vote(&self, x: &Event, y: &Event) -> bool {
        false
    }

    /// If event is famous (i.e., has had its fame decided by someone, and
    /// their vote was true).
    pub fn famous(&self, event: &Event) -> bool {
        false
    }

// ordering
    /// If event is famous and is the only famous witness in that round by that
    /// creator.
    pub fn unique_famous(&self, event: &Event) -> bool {
        false
    }

    /// If all known witnesses had their fame decided, for both round r and all
    /// earlier rounds.
    pub fn rounds_decided(&self, round: u32) -> bool {
        false
    }

    /// The round received for event.
    pub fn round_received(&self, event: &Event) -> u32 {
        0
    }

    /// The consensus timestamp for event.
    pub fn time_received(&self, event: &Event) -> SystemTime {
        SystemTime::now()
    }
}

/// The state of the consensus algorithm.
pub struct Consensus {
    /// Number of members in the population. Must be larger than one.
    population: u32,
    /// Frequency of coin rounds. Must be larger than two.
    freq_coin_rounds: u32,
    /// Hash graph containing all events.
    graph: HashGraph,
}

impl Consensus {
    /// Initialize consensus algorithm.
    pub fn new(population: u32, freq_coin_rounds: u32) -> Self {
        assert!(population > 1);
        assert!(freq_coin_rounds > 2);
        Self {
            population,
            freq_coin_rounds,
            graph: Default::default(),
        }
    }

    /// true if the set of events has more than 2n/3 events, and all have
    /// distinct authors.
    fn many_creators(&self, events: &HashSet<&Event>) -> bool {
        if events.len() <= (2 * self.population / 3) as usize {
            return false;
        }
        let mut authors = HashSet::new();
        for event in events {
            if authors.contains(event.author()) {
                return false;
            }
            authors.insert(event.author());
        }
        true
    }

}*/


/*
pub struct Event {
    round: u32,
    witness: bool,
    famous: bool,
    vote: bool,
}

    pub async fn send_sync(&self, peer: u32) {
        assert!(peer < self.population);
        // sync all known events
    }

    pub async fn receive_sync(&self) -> Vec<RawEvent> {
        // receive a sync
        //
    }

    pub fn divide_rounds(&self, raw: &RawEvent) -> Event {
        let self_parent = raw.self_parent.map(self.graph.get).unwrap_or(None);
        let other_parent = raw.other_parent.map(self.graph.get).unwrap_or(None);
        let self_parent_round = self_parent.map(|event| event.round).unwrap_or(1);
        let other_parent_round = other_parent.map(|event| event.round).unwrap_or(1);
        let parent_round = u32::max(self_parent_round, other_parent_round);
        let round = if false { // raw can strongly see more than 2n/3 round r witnesses
            round + 1
        } else {
            round
        };
        let witness = self_parent.is_none() || round > self_parent_round;
        Event {
            round,
            witness,
            self_parent,
            other_parent,
        }
    }

    pub async fn run(self: Arc<Self>) {
        task::spawn(async {
            loop {
                // select random member
                let peer = 0;
                self.send_sync(peer).await
            }
        });
        task::spawn(async {
            loop {
                let raw_events = self.receive_sync().await;
                for raw in &events {
                    let event = self.divide_rounds(raw);
                }
                // divide rounds
                // decide fame
                // find order
            }
        });
    }
}

    pub async fn receive_sync(&self, state: State) -> RawEvent {
        // receive raw event from peer
        RawEvent {
            payload: vec![].into_boxed_slice(),
            hashes: vec![],
            time: SystemTime::now(),
            author: 0,
            hash: multihash::Sha2_256::digest(&[]),
            signature: vec![].into_boxed_slice(),
        }
    }

    pub async fn run(self) {
        task::spawn(async {
            let mut rng = rand::thread_rng();
            loop {
                let state = self.state.read().await.clone();
                let peer = rng.gen() % state.population();
                self.send_sync(peer, state).await
            }
        });
        task::spawn(async {
            loop {
                let state = self.state.read().await.clone();
                let event = self.receive_sync(state).await;
                let state = self.graph.write().await.add_event(event);
                *self.state.write().await = state;
            }
        });
    }
*/
