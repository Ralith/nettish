use std::collections::{vec_deque, VecDeque};

/// Sequence of inputs transmitted to the server
///
/// Each input is associated with a wrapping *sequence number* used to identify when the server has
/// incorporated that input. Each simulation time step increments the sequence number by
/// one. Because they wrap, sequence numbers may be derived from a time-step counter with a larger
/// range by extracting the two least significant bytes.
#[derive(Debug, Clone)]
pub struct PredictionQueue<Input> {
    in_flight: VecDeque<Input>,
    next_sequence_number: u16,
}

impl<Input> PredictionQueue<Input> {
    pub fn new(next_sequence_number: u16) -> Self {
        Self {
            in_flight: VecDeque::new(),
            next_sequence_number,
        }
    }

    /// Sequence number that will obsolete the next input passed to [`record`](Self::record)
    pub fn next_sequence_number(&self) -> u16 {
        self.next_sequence_number
    }

    /// Track an input that's being sent to the server
    ///
    /// Should be called exactly once per simulation time step.
    pub fn record(&mut self, input: Input) {
        self.in_flight.push_back(input);
        self.next_sequence_number = self.next_sequence_number.wrapping_add(1);
    }

    /// Drop inputs transmitted at or before `sequence_number`
    ///
    /// Future inputs will be associated with sequence numbers greater than `sequence_number`,
    /// ensuring we re-synchronize after falling behind.
    pub fn reconcile(&mut self, sequence_number: u16) {
        let diff = self.next_sequence_number.wrapping_sub(sequence_number);
        if diff >= u16::MAX / 2 {
            // `sequence_number` is newer than anything we've recorded
            self.next_sequence_number = sequence_number.wrapping_add(1);
            self.in_flight.clear();
            return;
        }
        self.in_flight.drain(
            0..self
                .in_flight
                .len()
                .saturating_sub(diff.wrapping_sub(1) as usize),
        );
    }

    /// Iterate over stored inputs in the order they were [`record`](Self::record)ed
    pub fn iter(&self) -> vec_deque::Iter<'_, Input> {
        self.in_flight.iter()
    }
}

impl<'a, Input> IntoIterator for &'a PredictionQueue<Input> {
    type Item = &'a Input;
    type IntoIter = vec_deque::Iter<'a, Input>;

    fn into_iter(self) -> Self::IntoIter {
        self.in_flight.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        let mut q = PredictionQueue::<u16>::new(0);
        for i in 0..5 {
            q.record(i);
        }
        q.reconcile(0);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[1, 2, 3, 4]);
        q.reconcile(0);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[1, 2, 3, 4]);
        q.reconcile(2);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[3, 4]);
        q.reconcile(3);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[4]);
        q.reconcile(4);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[]);
        q.reconcile(4);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[]);
        q.record(5);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[5]);
    }

    #[test]
    fn wrap() {
        const START: u16 = u16::MAX - 1;
        let mut q = PredictionQueue::<u16>::new(START);
        for i in 0..5 {
            q.record(START.wrapping_add(i));
        }
        q.reconcile(START);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[u16::MAX, 0, 1, 2]);
        q.reconcile(START.wrapping_add(2));
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[1, 2]);
    }

    #[test]
    fn reordered() {
        let mut q = PredictionQueue::<u16>::new(0);
        for i in 0..5 {
            q.record(i);
        }
        q.reconcile(2);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[3, 4]);
        q.reconcile(0);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), &[3, 4]);
    }

    #[test]
    fn skipped() {
        let mut q = PredictionQueue::<u16>::new(0);
        for i in 0..5 {
            q.record(i);
        }
        q.reconcile(10);
        assert_eq!(
            q.iter().copied().collect::<Vec<_>>(),
            &[],
            "sequence numbers we haven't reached yet obsolete all inputs"
        );
        q.record(11);
        q.record(12);
        q.reconcile(11);
        assert_eq!(
            q.iter().copied().collect::<Vec<_>>(),
            &[12],
            "inputs are queued with future sequence numbers"
        );
    }
}
