use super::transaction::{Transaction, TransactionResult};
use crate::error::Error;
use crate::hash::{Hash, Hasher};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct Subscription {
    result: Option<TransactionResult>,
    wakers: Vec<Waker>,
}

impl Subscription {
    pub fn add_waker(&mut self, waker: Waker) {
        self.wakers.push(waker);
    }

    pub fn result(&self) -> Option<TransactionResult> {
        self.result.clone()
    }

    pub fn wake(&mut self, result: TransactionResult) {
        self.result = Some(result);
        for waker in self.wakers.drain(..) {
            waker.wake();
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TransactionQueue {
    subscriptions: HashMap<Hash, Arc<Mutex<Subscription>>>,
    queue: Vec<Transaction>,
}

impl TransactionQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_transaction(&mut self, tx: Transaction) -> Result<TransactionFuture, Error> {
        let bytes = bincode::serialize(&tx)?;
        let hash = Hasher::digest(bytes);
        self.queue.push(tx);
        let subscription = self.subscriptions.entry(hash).or_default().clone();
        Ok(TransactionFuture { subscription })
    }

    pub fn create_payload(&mut self) -> Box<[Transaction]> {
        std::mem::replace(&mut self.queue, vec![]).into_boxed_slice()
    }

    pub fn commit(&mut self, tx: &Transaction, result: TransactionResult) -> Result<(), Error> {
        let bytes = bincode::serialize(&tx)?;
        let hash = Hasher::digest(bytes);
        if let Some(subscription) = self.subscriptions.remove(&hash) {
            subscription.lock().unwrap().wake(result);
        }
        Ok(())
    }
}

pub struct TransactionFuture {
    subscription: Arc<Mutex<Subscription>>,
}

impl Future for TransactionFuture {
    type Output = TransactionResult;

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<Self::Output> {
        let mut subscription = self.subscription.lock().unwrap();
        if let Some(result) = subscription.result() {
            Poll::Ready(result)
        } else {
            subscription.add_waker(context.waker().clone());
            Poll::Pending
        }
    }
}
