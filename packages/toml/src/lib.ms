// toml — Lightweight, pure Mantis TOML parser.

use std;

pub type TomlEntry = struct {
    section String,
    key String,
    value String
}

pub type Toml = struct {
    entries Vec[TomlEntry]
}

// ── Helper Character Checks ────────────────────────────────────────────────

fn is_whitespace_byte(b u8) bool {
    if b == 32 { return true; }  // space
    if b == 9  { return true; }  // tab
    if b == 13 { return true; }  // carriage return
    if b == 10 { return true; }  // newline
    return false;
}

fn trim_slice(s Str) Str {
    let mut start = 0;
    loop {
        if start >= s.len {
            break;
        }
        let ptr = (s.pointer + start) as @u8;
        if !is_whitespace_byte(@(ptr)) {
            break;
        }
        start = start + 1;
    }

    if start >= s.len {
        return StrSlice { pointer: s.pointer, len: 0 };
    }

    let mut end = s.len;
    loop {
        if end <= start {
            break;
        }
        let ptr = (s.pointer + (end - 1)) as @u8;
        if !is_whitespace_byte(@(ptr)) {
            break;
        }
        end = end - 1;
    }

    return StrSlice {
        pointer: s.pointer + start,
        len: end - start
    };
}

fn strip_quotes(s Str) Str {
    let t = trim_slice(s);
    if t.len >= 2 {
        let first_ptr = t.pointer as @u8;
        let last_ptr = (t.pointer + (t.len - 1)) as @u8;
        let first = @(first_ptr);
        let last = @(last_ptr);
        if first == 34 {
            if last == 34 {
                return StrSlice {
                    pointer: t.pointer + 1,
                    len: t.len - 2
                };
            }
        }
        if first == 39 {
            if last == 39 {
                return StrSlice {
                    pointer: t.pointer + 1,
                    len: t.len - 2
                };
            }
        }
    }
    return t;
}

// ── TOML Parser Implementation ──────────────────────────────────────────────

impl Toml {
    pub fn new() Toml {
        return Toml {
            entries: Vec.with_capacity[TomlEntry](32)
        };
    }

    pub fn parse(src Str) Toml {
        let mut toml = Toml.new();
        let mut current_section = String.new();

        let mut line_start = 0;
        let mut i = 0;

        loop {
            if i > src.len {
                break;
            }

            let mut is_end = false;
            if i == src.len {
                is_end = true;
            } else {
                let ptr = (src.pointer + i) as @u8;
                if @(ptr) == 10 { // \n
                    is_end = true;
                }
            }

            if is_end {
                let line_len = i - line_start;
                if line_len > 0 {
                    let raw_line = StrSlice {
                        pointer: src.pointer + line_start,
                        len: line_len
                    };
                    let line = trim_slice(raw_line);

                    if line.len > 0 {
                        let first_byte_ptr = line.pointer as @u8;
                        let first_b = @(first_byte_ptr);

                        // Skip comments
                        if first_b != 35 { // #
                            if first_b == 91 { // [ section header
                                let last_b_ptr = (line.pointer + (line.len - 1)) as @u8;
                                if @(last_b_ptr) == 93 { // ]
                                    let sec_name = StrSlice {
                                        pointer: line.pointer + 1,
                                        len: line.len - 2
                                    };
                                    let sec_trimmed = trim_slice(sec_name);
                                    current_section = String.from_str(sec_trimmed);
                                }
                            } else {
                                // Key = Value pair
                                let mut eq_pos = -1;
                                let mut j = 0;
                                loop {
                                    if j >= line.len {
                                        break;
                                    }
                                    let b_ptr = (line.pointer + j) as @u8;
                                    if @(b_ptr) == 61 { // =
                                        eq_pos = j;
                                        break;
                                    }
                                    j = j + 1;
                                }

                                if eq_pos > 0 {
                                    let k_slice = StrSlice {
                                        pointer: line.pointer,
                                        len: eq_pos
                                    };
                                    let v_slice = StrSlice {
                                        pointer: line.pointer + (eq_pos + 1),
                                        len: line.len - (eq_pos + 1)
                                    };

                                    let k_trimmed = trim_slice(k_slice);
                                    let v_trimmed = trim_slice(v_slice);

                                    // Handle inline tables: { path = "..." }
                                    if v_trimmed.len >= 2 {
                                        let v_first_ptr = v_trimmed.pointer as @u8;
                                        let v_last_ptr = (v_trimmed.pointer + (v_trimmed.len - 1)) as @u8;
                                        if @(v_first_ptr) == 123 {
                                          if @(v_last_ptr) == 125 { // { and }
                                            let inner = StrSlice {
                                                pointer: v_trimmed.pointer + 1,
                                                len: v_trimmed.len - 2
                                            };
                                            let inner_trimmed = trim_slice(inner);
                                            // Look for sub-key = sub-val
                                            let mut sub_eq = -1;
                                            let mut k_idx = 0;
                                            loop {
                                                if k_idx >= inner_trimmed.len {
                                                    break;
                                                }
                                                let sb_ptr = (inner_trimmed.pointer + k_idx) as @u8;
                                                if @(sb_ptr) == 61 {
                                                    sub_eq = k_idx;
                                                    break;
                                                }
                                                k_idx = k_idx + 1;
                                            }
                                            if sub_eq > 0 {
                                                let sub_k = trim_slice(StrSlice {
                                                    pointer: inner_trimmed.pointer,
                                                    len: sub_eq
                                                });
                                                let sub_v = strip_quotes(StrSlice {
                                                    pointer: inner_trimmed.pointer + (sub_eq + 1),
                                                    len: inner_trimmed.len - (sub_eq + 1)
                                                });

                                                let mut compound_k = String.from_str(k_trimmed);
                                                compound_k.push_byte(46); // .
                                                compound_k.push_str(sub_k);

                                                let entry = TomlEntry {
                                                    section: String.from_str(current_section.as_str()),
                                                    key: compound_k,
                                                    value: String.from_str(sub_v)
                                                };
                                                toml.entries.push(entry);
                                            }
                                          }
                                        }
                                    }

                                    let final_val = strip_quotes(v_trimmed);
                                    let entry = TomlEntry {
                                        section: String.from_str(current_section.as_str()),
                                        key: String.from_str(k_trimmed),
                                        value: String.from_str(final_val)
                                    };
                                    toml.entries.push(entry);
                                }
                            }
                        }
                    }
                }
                line_start = i + 1;
            }
            i = i + 1;
        }

        return toml;
    }

    pub fn get(self @Toml, section Str, key Str) Option[String] {
        let mut i = 0;
        loop {
            if i >= self.entries.len() {
                break;
            }
            let entry = self.entries.get(i);
            if entry.section.as_str() == section {
                if entry.key.as_str() == key {
                    return Option.Some[String](entry.value);
                }
            }
            i = i + 1;
        }
        return Option.None[String]();
    }

    pub fn get_or(self @Toml, section Str, key Str, default_val Str) String {
        let opt = self.get(section, key);
        if opt.is_some() {
            return opt.unwrap();
        }
        return String.from_str(default_val);
    }

    pub fn get_int(self @Toml, section Str, key Str, default_val i64) i64 {
        let opt = self.get(section, key);
        if opt.is_some() {
            let val_str = opt.unwrap();
            let s = val_str.as_str();
            let mut result: i64 = 0;
            let mut j = 0;
            let mut negative = false;
            if s.len > 0 {
                let first = @(s.pointer as @u8);
                if first == 45 { // -
                    negative = true;
                    j = 1;
                }
            }
            loop {
                if j >= s.len {
                    break;
                }
                let b = @((s.pointer + j) as @u8);
                if b >= 48 & b <= 57 { // 0-9
                    result = result * 10 + ((b - 48) as i64);
                }
                j = j + 1;
            }
            if negative {
                return -result;
            }
            return result;
        }
        return default_val;
    }

    pub fn get_bool(self @Toml, section Str, key Str, default_val bool) bool {
        let opt = self.get(section, key);
        if opt.is_some() {
            let s = opt.unwrap();
            if s.as_str() == "true" {
                return true;
            }
            if s.as_str() == "false" {
                return false;
            }
        }
        return default_val;
    }

    /// Returns all dependency directory paths declared under `[dependencies]`.
    pub fn get_dep_paths(self @Toml) Vec[String] {
        let mut result = Vec.new[String]();
        let mut i = 0;
        loop {
            if i >= self.entries.len() {
                break;
            }
            let entry = self.entries.get(i);
            if entry.section.as_str() == "dependencies" {
                let k = entry.key.as_str();
                // Check if key ends with ".path" or is a path
                if k.len >= 5 {
                    let suffix = StrSlice {
                        pointer: k.pointer + (k.len - 5),
                        len: 5
                    };
                    if suffix == ".path" {
                        result.push(entry.value);
                    }
                }
            }
            i = i + 1;
        }
        return result;
    }
}
