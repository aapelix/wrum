use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
pub enum Insert {
    Ok,
    Dup,
    TooFar,
}

pub struct Window<T> {
    base: u16,
    slots: VecDeque<Option<T>>,
    cap: usize,
}

impl<T> Window<T> {
    pub fn new(cap: usize) -> Self {
        debug_assert!(cap < 0x8000);
        Self {
            base: 0,
            slots: VecDeque::new(),
            cap,
        }
    }
    #[inline]
    pub fn base(&self) -> u16 {
        self.base
    }
    pub fn insert(&mut self, seq: u16, item: T) -> Insert {
        let d = seq.wrapping_sub(self.base) as usize;
        if d >= 0x8000 {
            return Insert::Dup;
        }
        if d >= self.cap {
            return Insert::TooFar;
        }
        if self.slots.len() <= d {
            self.slots.resize_with(d + 1, || None);
        }
        if self.slots[d].is_some() {
            return Insert::Dup;
        }
        self.slots[d] = Some(item);
        Insert::Ok
    }
    #[inline]
    pub fn advance(&mut self) {
        self.slots.pop_front();
        self.base = self.base.wrapping_add(1);
    }
    pub fn pop_ready(&mut self) -> Option<(u16, T)> {
        if matches!(self.slots.front(), Some(Some(_))) {
            let item = self.slots.pop_front()??;
            let s = self.base;
            self.base = self.base.wrapping_add(1);
            Some((s, item))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorders_and_dedupes() {
        let mut w = Window::new(16);
        assert_eq!(w.insert(2, 'c'), Insert::Ok);
        assert_eq!(w.insert(2, 'c'), Insert::Dup);
        assert_eq!(w.insert(1, 'b'), Insert::Ok);
        assert!(w.pop_ready().is_none());
        assert_eq!(w.insert(0, 'a'), Insert::Ok);
        let got: Vec<_> = std::iter::from_fn(|| w.pop_ready()).collect();
        assert_eq!(got, vec![(0, 'a'), (1, 'b'), (2, 'c')]);
        assert_eq!(w.insert(1, 'x'), Insert::Dup);
        assert_eq!(w.insert(100, 'x'), Insert::TooFar);
    }

    #[test]
    fn wraps() {
        let mut w = Window::new(16);
        w.base = 65534;
        assert_eq!(w.insert(1, 'c'), Insert::Ok);
        assert_eq!(w.insert(65535, 'b'), Insert::Ok);
        assert_eq!(w.insert(65534, 'a'), Insert::Ok);
        assert_eq!(w.insert(0, 'z'), Insert::Ok);
        let got: Vec<_> = std::iter::from_fn(|| w.pop_ready()).map(|x| x.1).collect();
        assert_eq!(got, vec!['a', 'b', 'z', 'c']);
    }
}
