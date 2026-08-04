type Option[T] = enum {
    Some(T),
    None
}

impl[T] Option[T] {
    fn is_some(self) bool {
        match self {
            Option.Some(_) : { return true; },
            Option.None : { return false; }
        }
    }

    fn is_none(self) bool {
        return !self.is_some();
    }

    fn unwrap(self) T {
        match self {
            Option.Some(val) : { return val; },
            Option.None : { panic("called Option.unwrap() on a None value"); }
        }
    }

    fn unwrap_or(self, default_val T) T {
        match self {
            Option.Some(val) : { return val; },
            Option.None : { return default_val; }
        }
    }
}
