use core::{
    future::Future,
    task::{Poll, Waker},
};

use crate::trace;

use super::MpMcQueue;

pub struct DequeueFuture<'queue, T, const W: usize, const N: usize>
where
    T: Unpin,
{
    inner: &'queue MpMcQueue<T, W, N>,
    dequeued_value: Option<T>,
}

impl<'queue, T, const W: usize, const N: usize> DequeueFuture<'queue, T, W, N>
where
    T: Unpin,
{
    pub const fn new(queue: &'queue MpMcQueue<T, W, N>) -> Self {
        Self {
            inner: queue,
            dequeued_value: None,
        }
    }

    fn try_wake_enqueuers(&self) -> bool {
        self.inner
            .wakers
            .enqueue_wakers
            .try_lock(|wks| wks.iter_mut().for_each(|wk| wk.wake()))
            .is_some()
    }

    fn register_waker(&mut self, waker: &Waker) -> bool {
        let res = self.inner.wakers.dequeue_wakers.try_lock(|wks| {
            wks.iter_mut()
                .find(|wk| wk.is_empty())
                .map(|wk| wk.register(waker))
                .is_some()
        });

        res == Some(true)
    }
}

impl<T, const W: usize, const N: usize> Future for DequeueFuture<'_, T, W, N>
where
    T: Unpin,
{
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let try_wake_producer = |me: &mut Self, value| {
            if me.try_wake_enqueuers() {
                return Poll::Ready(value);
            } else {
                me.dequeued_value = Some(value);
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
        };

        trace!("Poll consumer");
        let me = self.get_mut();
        let con = &mut me.inner;

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
            if !me.register_waker(cx.waker()) {
                cx.waker().wake_by_ref()
            }
            Poll::Pending
        }
    }
}
