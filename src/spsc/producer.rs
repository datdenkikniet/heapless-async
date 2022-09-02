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
}

pub struct ProducerFuture<'producer, 'queue, T, const N: usize>
where
    T: Unpin,
{
    producer: &'producer mut Producer<'queue, T, N>,
    value_to_enqueue: Option<T>,
}

impl<T, const N: usize> ProducerFuture<'_, '_, T, N>
where
    T: Unpin,
{
    fn try_wake_consumer(&mut self) -> bool {
        if self
            .producer
            .consumer_waker
            .try_lock(|wk| {
                trace!("Waking consumer");
                wk.wake();
            })
            .is_some()
        {
            true
        } else {
            debug!("Failed to wake consumer");
            false
        }
    }

    fn register_waker(&mut self, waker: &Waker) -> bool {
        if self
            .producer
            .producer_waker
            .try_lock(|wk| {
                wk.register(waker);
                trace!("Registered producer waker");
            })
            .is_some()
        {
            true
        } else {
            trace!("Failed to register producer waker");
            false
        }
    }
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
            if me.try_wake_consumer() {
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

        if !me.register_waker(cx.waker()) {
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    }
}
