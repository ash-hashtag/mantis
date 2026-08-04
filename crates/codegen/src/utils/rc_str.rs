use std::{
    fmt::Display,
    ops::{Deref, Range},
    rc::Rc,
};

#[derive(Debug, Clone)]
enum Kind {
    Static(&'static str),
    Shared(Rc<str>),
}

impl Deref for Kind {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Kind::Static(s) => s,
            Kind::Shared(s) => s.deref(),
        }
    }
}

impl PartialOrd for RcStr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl Ord for RcStr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct RcStr {
    inner: Kind,
    range: Range<usize>,
}

impl std::cmp::Eq for RcStr {}

impl std::hash::Hash for RcStr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl PartialEq for RcStr {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl RcStr {
    // pub fn new(s: String) -> Self {
    //     let range = 0..s.len();
    //     let inner: Rc<str> = s.into();

    //     Self { inner, range }
    // }
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

    pub fn trim(&self) -> Self {
        if let Some((word_start, _)) = self.char_indices().find(|(_, x)| !x.is_whitespace()) {
            if let Some((word_end, _)) = self.char_indices().rev().find(|(_, x)| !x.is_whitespace())
            {
                return self.slice(word_start..word_end + 1).unwrap_or(self.clone());
            }
        }
        self.clone()
    }

    pub fn as_str(&self) -> &str {
        &self.inner[self.range.clone()]
    }

    pub fn len(&self) -> usize {
        self.range.len()
    }

    // pub fn merge(&self, other: Self) -> Option<Self> {
    //     let is_same_ref = Rc::eq(&self.inner, &other.inner);
    //     if is_same_ref && self.range.end == other.range.start {
    //         let range = self.range.start..other.range.end;
    //         Some(Self {
    //             inner: self.inner.clone(),
    //             range,
    //         })
    //     } else {
    //         None
    //     }
    // }
}

impl Deref for RcStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Display for RcStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl From<String> for RcStr {
    fn from(value: String) -> Self {
        Self {
            range: 0..value.len(),
            inner: Kind::Shared(value.into()),
        }
    }
}

// impl From<&str> for RcStr {
//     fn from(value: &str) -> Self {
//         Self::new(value.into())
//     }
// }

impl From<&'static str> for RcStr {
    fn from(value: &'static str) -> Self {
        Self {
            range: 0..value.len(),
            inner: Kind::Static(value),
        }
    }
}
