use core::{
    future::Future,
    task::{Poll, Waker},
};

use super::MpMcQueue;

pub struct EnqueueFuture<'queue, T, const W: usize, const N: usize>
where
    T: Unpin,
{
    inner: &'queue MpMcQueue<T, W, N>,
    value_to_enqueue: Option<T>,
}

impl<'queue, T, const W: usize, const N: usize> EnqueueFuture<'queue, T, W, N>
where
    T: Unpin,
{
    pub fn new(queue: &'queue MpMcQueue<T, W, N>, value: T) -> Self {
        Self {
            inner: queue,
            value_to_enqueue: Some(value),
        }
    }

    fn try_wake_dequeuers(&self) -> bool {
        self.inner
            .wakers
            .dequeue_wakers
            .try_lock(|wks| wks.iter_mut().for_each(|wk| wk.wake()))
            .is_some()
    }

    fn register_waker(&mut self, waker: &Waker) -> bool {
        let res = self.inner.wakers.enqueue_wakers.try_lock(|wks| {
            wks.iter_mut()
                .find(|wk| wk.is_empty())
                .map(|wk| wk.register(waker))
                .is_some()
        });

        res == Some(true)
    }
}

impl<T, const W: usize, const N: usize> Future for EnqueueFuture<'_, T, W, N>
where
    T: Unpin,
{
    type Output = ();

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let try_wake_dequeuers = |me: &mut Self| {
            if me.try_wake_dequeuers() {
                Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        };

        let me = self.get_mut();

        let value = if let Some(value) = me.value_to_enqueue.take() {
            value
        } else {
            return try_wake_dequeuers(me);
        };

        let failed_to_enqueue_value = if let Err(value) = me.inner.inner.enqueue(value) {
            value
        } else {
            return try_wake_dequeuers(me);
        };

        me.value_to_enqueue = Some(failed_to_enqueue_value);
        if !me.register_waker(cx.waker()) {
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    }
}
