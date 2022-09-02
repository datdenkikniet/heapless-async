use core::{
    future::Future,
    task::{Poll, Waker},
};

use heapless::spsc::Consumer as HConsumer;

use crate::{lock::Lock, log::*, waker::WakerRegistration};

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
    /// This function may block for a while as result of contention of
    /// a lock between the Producer/Consumer.
    pub fn try_dequeue(&mut self) -> Option<T> {
        let value = self.inner.dequeue();

        if value.is_some() {
            while !self.try_wake_producer() {}
        }

        value
    }

    fn try_wake_producer(&mut self) -> bool {
        if self
            .producer_waker
            .try_lock(|wk| {
                trace!("Waking producer");
                wk.wake()
            })
            .is_some()
        {
            true
        } else {
            trace!("Failed to wake producer");
            false
        }
    }

    fn try_register_waker(&mut self, waker: &Waker) -> bool {
        if self
            .consumer_waker
            .try_lock(|wk| {
                wk.register(waker);
                trace!("Registered consumer waker.");
            })
            .is_none()
        {
            trace!("Failed to register consumer waker.");
            false
        } else {
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
