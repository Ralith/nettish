use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// A jitter-tolerant queue of inputs received from a client
///
/// Clients send a stream of input roughly at tickrate, but with an undefined time offset causing
/// ticks to line up imperfectly. If they happen to line up very closely, or if tickrate is
/// sufficiently high, then network jitter might cause frequent variation in whether an input is
/// received just before or just after the simulation is stepped. If we applied inputs ASAP, this
/// would make it impossible for the client to accurately predict the effects of its input, leading
/// to severe visual jitter.
///
/// To correct this, we wait a certain amount of time after receiving the first input, and only then
/// begin consuming one input per tick. This ensures that each input can be late by that amount of
/// time without disrupting the client's prediction. If we nonetheless run out of inputs, it's
/// likely that the client fell behind, e.g. due to a temporary hang, clock drift, or a change in
/// the network path, so we wait again to recover the margin for error.
pub struct InputQueue<T> {
    queue: VecDeque<T>,
    /// Time at which the first input in the latest uninterrupted sequence was received
    epoch: Option<Instant>,
}

impl<T> InputQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a new input
    ///
    /// Called immediately on receipt
    pub fn push(&mut self, max: usize, input: T, now: Instant) {
        if self.queue.len() == max {
            // Overrun
            self.queue.pop_front();
        }
        self.queue.push_back(input);
        if self.epoch.is_none() {
            self.epoch = Some(now);
        }
    }

    /// Obtain the input for the next simulation step
    ///
    /// Must be called immediately prior to the step. Inputs may be `push`ed late by at most `delay`
    /// before an under-run will occur; i.e. `delay` is the amount of time after the first (but not
    /// necessarily future) input in a given uninterrupted sequence of inputs we must wait before
    /// beginning to consume inputs.
    pub fn take(&mut self, now: Instant, delay: Duration) -> Option<T> {
        if now - self.epoch? < delay {
            // The first input hasn't aged long enough; try again later!
            return None;
        }
        let result = self.queue.pop_front();
        if result.is_none() {
            // Queue under-run; the client may have fallen behind, so we need to re-establish our
            // margin for error.
            self.epoch = None;
        }
        result
    }

    /// Number of inputs queued
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether any inputs are queued
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl<T> Default for InputQueue<T> {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            epoch: None,
        }
    }
}
