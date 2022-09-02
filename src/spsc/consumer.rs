use core::{
    future::Future,
    task::{Poll, Waker},
};

use heapless::spsc::Consumer as HConsumer;

use crate::{lock::Lock, log::*, waker::WakerRegistration};

/// This error may be returned by [`Consumer::try_dequeue`].
pub enum ConsumerError {
    WouldBlock,
    Empty,
}

/// An async consumer
pub struct Consumer<'queue, T, const N: usize>
where
    T: Unpin,
{
    inner: HConsumer<'queue, T, N>,
    producer_waker: &'queue Lock<WakerRegistration>,
    consumer_waker: &'queue Lock<WakerRegistration>,
}

impl<'queue, T, const N: usize> Consumer<'queue, T, N>
where
    T: Unpin,
{
    pub(crate) fn new(
        consumer: HConsumer<'queue, T, N>,
        producer_waker: &'queue Lock<WakerRegistration>,
        consumer_waker: &'queue Lock<WakerRegistration>,
    ) -> Self {
        Self {
            inner: consumer,
            producer_waker,
            consumer_waker,
        }
    }

    /// Check if there are any items to dequeue.
    ///
    /// When this returns true, at least the first subsequent [`Self::dequeue`] will succeed immediately
    pub fn ready(&self) -> bool {
        self.inner.ready()
    }

    /// Returns the maximum number of elements the queue can hold
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Returns the amount of elements currently in the queue
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Dequeue an item from the backing queue.
    ///
    /// The returned future only resolves once an item was succesfully
    /// dequeued.
    pub fn dequeue<'me>(&'me mut self) -> ConsumerFuture<'me, 'queue, T, N> {
        ConsumerFuture {
            consumer: self,
            dequeued_value: None,
        }
    }

    /// Attempt to dequeue an item from the backing queue.
    ///
    /// If [`ConsumerError::WouldBlock`] is returned, the [`Producer`](super::Producer)
    /// has a lock on it's waker which prevents this consumer from properly waking it.
    pub fn try_dequeue(&mut self) -> Result<T, ConsumerError> {
        if !self.try_wake_producer() {
            return Err(ConsumerError::WouldBlock);
        }

        if let Some(val) = self.inner.dequeue() {
            Ok(val)
        } else {
            Err(ConsumerError::Empty)
        }
    }

    /// Try to wake the [`Producer`](super::Producer) associated with the backing queue.
    ///
    /// Returns true if the waker was waked succesfully.
    fn try_wake_producer(&mut self) -> bool {
        if self.producer_waker.try_lock(|wk| wk.wake()).is_some() {
            trace!("Waking producer");
            true
        } else {
            trace!("Failed to wake producer");
            false
        }
    }

    /// Try to register `waker` as the waker for this [`Consumer`]
    ///
    /// Returns true if the waker was registered succesfully.
    fn try_register_waker(&mut self, waker: &Waker) -> bool {
        let cons_waker = &mut self.consumer_waker;
        if cons_waker.try_lock(|wk| wk.register(waker)).is_none() {
            trace!("Failed to register consumer waker.");
            false
        } else {
            trace!("Registered consumer waker.");
            true
        }
    }
}

pub struct ConsumerFuture<'consumer, 'queue, T, const N: usize>
where
    T: Unpin,
{
    consumer: &'consumer mut Consumer<'queue, T, N>,
    dequeued_value: Option<T>,
}

impl<T, const N: usize> Future for ConsumerFuture<'_, '_, T, N>
where
    T: Unpin,
{
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let try_wake_producer = |me: &mut Self, value| {
            if me.consumer.try_wake_producer() {
                return Poll::Ready(value);
            } else {
                me.dequeued_value = Some(value);
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        debug!("Poll consumer");
        let me = self.get_mut();
        let con = &mut me.consumer;

        if let Some(value) = me.dequeued_value.take() {
            // Try to wake the producer because we managed to
            // dequeue a value
            return try_wake_producer(me, value);
        }

        me.dequeued_value = con.inner.dequeue();
        if let Some(value) = me.dequeued_value.take() {
            // Try to wake the producer because we managed to
            // dequeue a value
            try_wake_producer(me, value)
        } else {
            if !me.consumer.try_register_waker(cx.waker()) {
                cx.waker().wake_by_ref()
            }
            Poll::Pending
        }
    }
}
