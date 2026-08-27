pub type Result[T, E] = enum {
    Ok(T),
    Err(E)
}

impl[T, E] Result[T, E] {
    pub fn is_ok(self) bool {
        match self {
            Result.Ok(_) : { return true; },
            Result.Err(_) : { return false; }
        }
    }

    pub fn is_err(self) bool {
        return !self.is_ok();
    }

    pub fn unwrap(self) T {
        match self {
            Result.Ok(val) : { return val; },
            Result.Err(_) : { panic("called Result.unwrap() on an Err value"); }
        }
    }

    pub fn unwrap_err(self) E {
        match self {
            Result.Ok(_) : { panic("called Result.unwrap_err() on an Ok value"); },
            Result.Err(err) : { return err; }
        }
    }
}
