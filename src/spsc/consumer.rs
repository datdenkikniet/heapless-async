use core::{
    future::Future,
    task::{Poll, Waker},
};

use heapless::spsc::Consumer as HConsumer;

use crate::{log::*, mutex::Mutex, waker::WakerRegistration};

/// This error may be returned by [`Consumer::try_dequeue`].
pub enum ConsumerError<T> {
    /// Waking the producer would block.
    ///
    /// Retrying is possible, but is highly discouraged as it
    /// may block forever.
    ///
    /// It only works if releasing the lock held by the producer
    /// preempts the code that performs the retries.    
    WouldBlock(Option<T>),
    /// The queue is empty.
    ///
    /// Retrying is possible, but is highly discouraged as it
    /// may block forever.
    ///
    /// It only works if enqueueing an item into the backing
    /// queue preempts the code that performs the retries.
    Empty,
}

/// An async consumer
pub struct Consumer<'queue, T, const N: usize>
where
    T: Unpin,
{
    inner: HConsumer<'queue, T, N>,
    producer_waker: &'queue Mutex<WakerRegistration>,
    consumer_waker: &'queue Mutex<WakerRegistration>,
}

impl<'queue, T, const N: usize> Consumer<'queue, T, N>
where
    T: Unpin,
{
    pub(crate) fn new(
        consumer: HConsumer<'queue, T, N>,
        producer_waker: &'queue Mutex<WakerRegistration>,
        consumer_waker: &'queue Mutex<WakerRegistration>,
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
    /// has a lock on it's waker which prevents this producer from properly waking it.
    /// In such a case, the application can attempt to re-wake the [`Producer`](super::Producer)
    /// by calling [`Consumer::try_wake_producer`].
    pub fn try_dequeue(&mut self) -> Result<T, ConsumerError<T>> {
        let res = if let Some(val) = self.inner.dequeue() {
            Ok(val)
        } else {
            Err(ConsumerError::Empty)
        };

        if !self.try_wake_producer() {
            return Err(ConsumerError::WouldBlock(res.ok()));
        }

        res
    }

    /// Try to wake the [`Producer`](super::Producer) associated with the backing queue.
    ///
    /// Returns true if the waker was waked succesfully.
    pub fn try_wake_producer(&mut self) -> bool {
        if let Some(mut wk) = self.producer_waker.try_lock() {
            wk.wake();
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
        if let Some(mut wk) = self.consumer_waker.try_lock() {
            wk.register(waker);
            trace!("Registered consumer waker.");
            true
        } else {
            trace!("Failed to register consumer waker.");
            false
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
