use core::{
    future::Future,
    task::{Poll, Waker},
};

use heapless::spsc::Producer as HProducer;

use crate::{lock::Lock, log::*, waker::WakerRegistration};

/// An async producer
pub struct Producer<'queue, T, const N: usize>
where
    T: Unpin,
{
    inner: HProducer<'queue, T, N>,
    producer_waker: &'queue Lock<WakerRegistration>,
    consumer_waker: &'queue Lock<WakerRegistration>,
}

impl<'queue, T, const N: usize> Producer<'queue, T, N>
where
    T: Unpin,
{
    pub(crate) fn new(
        producer: HProducer<'queue, T, N>,
        producer_waker: &'queue Lock<WakerRegistration>,
        consumer_waker: &'queue Lock<WakerRegistration>,
    ) -> Self {
        Self {
            inner: producer,
            producer_waker,
            consumer_waker,
        }
    }

    /// Check if an item can be enqueued.
    ///
    /// If this returns true, at least the first subsequent [`Self::enqueue`] will succeed
    /// immediately.
    pub fn ready(&self) -> bool {
        self.inner.ready()
    }

    /// Returns the maximum number of elements the queue can hold.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Returns the amount of elements currently in the queue.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Enqueue `value` into the backing queue.
    ///
    /// The returned Future only resolves once the value was
    /// succesfully enqueued.
    pub fn enqueue<'me>(&'me mut self, value: T) -> ProducerFuture<'me, 'queue, T, N> {
        let value = self.inner.enqueue(value).err();
        ProducerFuture {
            producer: self,
            value_to_enqueue: value,
        }
    }

    /// Try to enqueue `value` into the backing queue.
    ///
    /// This function may block for a while as result of contention of
    /// a lock between the [`Producer`] and [`Consumer`](super::Consumer).
    pub fn try_enqueue(&mut self, value: T) -> Result<(), T> {
        let result = self.inner.enqueue(value);

        if result.is_ok() {
            while !self.try_wake_consumer() {}
        }

        result
    }

    /// Try to wake the [`Consumer`](super::Consumer) associated with the backing queue.
    ///
    /// Returns true if the waker was waked succesfully.
    fn try_wake_consumer(&mut self) -> bool {
        let cons_waker = &mut self.consumer_waker;
        if cons_waker.try_lock(|wk| wk.wake()).is_some() {
            trace!("Waking consumer");
            true
        } else {
            debug!("Failed to wake consumer");
            false
        }
    }

    /// Try to register `waker` as the waker for this [`Producer`]
    ///
    /// Returns true if the waker was registered succesfully.
    fn try_register_waker(&mut self, waker: &Waker) -> bool {
        let prod_waker = self.producer_waker;
        if prod_waker.try_lock(|wk| wk.register(waker)).is_some() {
            trace!("Registered producer waker");
            true
        } else {
            trace!("Failed to register producer waker");
            false
        }
    }
}

pub struct ProducerFuture<'producer, 'queue, T, const N: usize>
where
    T: Unpin,
{
    producer: &'producer mut Producer<'queue, T, N>,
    value_to_enqueue: Option<T>,
}

impl<T, const N: usize> Future for ProducerFuture<'_, '_, T, N>
where
    T: Unpin,
{
    type Output = ();

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        trace!("Poll producer");
        let try_wake_consumer = |me: &mut Self| {
            if me.producer.try_wake_consumer() {
                return Poll::Ready(());
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        };

        let me = self.get_mut();
        let prod = &mut me.producer;
        let val_to_enqueue = &mut me.value_to_enqueue;

        let value = if let Some(value) = val_to_enqueue.take() {
            value
        } else {
            // Try to wake the consumer because we've enqueued our value
            return try_wake_consumer(me);
        };

        let failed_enqueue_value = if let Some(value) = prod.inner.enqueue(value).err() {
            value
        } else {
            // Try to wake the consumer because we've enqueued our value
            return try_wake_consumer(me);
        };

        me.value_to_enqueue = Some(failed_enqueue_value);

        if !me.producer.try_register_waker(cx.waker()) {
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    }
}
