use std::{
    fmt::Display,
    ops::{Deref, Range},
    rc::Rc,
};

#[derive(Debug)]
pub struct RcVec<T> {
    inner: Rc<[T]>,
    range: Range<usize>,
}

impl<T> RcVec<T> {
    pub fn empty() -> Self {
        Self {
            range: 0..0,
            inner: Rc::new([]),
        }
    }
    pub fn new(s: Vec<T>) -> Self {
        let range = 0..s.len();
        let inner: Rc<[T]> = s.into();

        Self { inner, range }
    }
    pub fn slice(&self, range: Range<usize>) -> Option<Self> {
        if self.range.start + range.end > self.inner.len() {
            return None;
        }
        let range = (self.range.start + range.start)..(self.range.start + range.end);
        Some(Self {
            inner: self.inner.clone(),
            range,
        })
    }

    pub fn as_slice(&self) -> &[T] {
        &self.inner[self.range.clone()]
    }

    pub fn len(&self) -> usize {
        self.range.len()
    }
}

impl<T> Deref for RcVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> Clone for RcVec<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            range: self.range.clone(),
        }
    }
}

use std::fmt::Debug;

use super::rc_str::RcStr;

impl<T> Display for RcVec<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_slice())
    }
}
impl<T> From<Vec<T>> for RcVec<T> {
    fn from(value: Vec<T>) -> Self {
        Self::new(value)
    }
}

impl<T> From<&[T]> for RcVec<T>
where
    T: Clone,
{
    fn from(value: &[T]) -> Self {
        Self::new(Vec::from(value))
    }
}

impl<T> From<Rc<[T]>> for RcVec<T> {
    fn from(value: Rc<[T]>) -> Self {
        Self {
            range: 0..value.len(),
            inner: value,
        }
    }
}
