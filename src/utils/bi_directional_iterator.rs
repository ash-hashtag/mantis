use std::{ops::Deref, path::Iter};

use super::rc_vec::RcVec;

pub struct BiDerectionalIterator<T> {
    inner: RcVec<T>,
    index: usize,
}

impl<T> BiDerectionalIterator<T> {
    pub fn new(inner: RcVec<T>, index: usize) -> Option<Self> {
        let _ = inner.get(index)?;
        Some(Self { inner, index })
    }

    pub fn get(&self) -> &T {
        &self.inner[self.index]
    }

    pub fn prev(&self) -> Option<Self> {
        self.prev_n(1)
    }

    pub fn next(&self) -> Option<Self> {
        self.next_n(1)
    }

    pub fn next_n(&self, n: usize) -> Option<Self> {
        Self::new(self.inner.clone(), self.index + n)
    }

    pub fn prev_n(&self, n: usize) -> Option<Self> {
        Self::new(self.inner.clone(), self.index.checked_sub(n)?)
    }
}

impl<T> Clone for BiDerectionalIterator<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            index: self.index.clone(),
        }
    }
}

impl<T> Deref for BiDerectionalIterator<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> Iterator for BiDerectionalIterator<T> {
    type Item = BiDerectionalIterator<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let _ = self.inner.get(self.index + 1)?;
        self.index += 1;
        Some(self.clone())
    }
}
