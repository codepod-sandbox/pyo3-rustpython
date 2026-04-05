//! Simplified string cache for RustPython — no FFI, just PyString::new().

use std::sync::{Mutex, MutexGuard, OnceLock};

use ahash::random_state::RandomState;
use pyo3::prelude::*;
use pyo3::types::PyString;

use jiter::string_decoder::StringOutput;

#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub enum StringCacheMode {
    #[default]
    All,
    Keys,
    None,
}

impl From<bool> for StringCacheMode {
    fn from(mode: bool) -> Self {
        if mode {
            Self::All
        } else {
            Self::None
        }
    }
}

pub trait StringMaybeCache {
    fn get_key<'py>(py: Python<'py>, string_output: StringOutput<'_, '_>) -> Bound<'py, PyString>;

    fn get_value<'py>(py: Python<'py>, string_output: StringOutput<'_, '_>) -> Bound<'py, PyString> {
        Self::get_key(py, string_output)
    }
}

pub struct StringCacheAll;

impl StringMaybeCache for StringCacheAll {
    fn get_key<'py>(py: Python<'py>, string_output: StringOutput<'_, '_>) -> Bound<'py, PyString> {
        cached_py_string(py, string_output.as_str())
    }
}

pub struct StringCacheKeys;

impl StringMaybeCache for StringCacheKeys {
    fn get_key<'py>(py: Python<'py>, string_output: StringOutput<'_, '_>) -> Bound<'py, PyString> {
        cached_py_string(py, string_output.as_str())
    }

    fn get_value<'py>(py: Python<'py>, string_output: StringOutput<'_, '_>) -> Bound<'py, PyString> {
        Bound::<PyString>::new(py, string_output.as_str())
    }
}

pub struct StringNoCache;

impl StringMaybeCache for StringNoCache {
    fn get_key<'py>(py: Python<'py>, string_output: StringOutput<'_, '_>) -> Bound<'py, PyString> {
        Bound::<PyString>::new(py, string_output.as_str())
    }
}

static STRING_CACHE: OnceLock<Mutex<PyStringCache>> = OnceLock::new();

#[inline]
fn get_string_cache() -> MutexGuard<'static, PyStringCache> {
    match STRING_CACHE
        .get_or_init(|| Mutex::new(PyStringCache::default()))
        .lock()
    {
        Ok(cache) => cache,
        Err(poisoned) => {
            let mut cache = poisoned.into_inner();
            cache.clear();
            cache
        }
    }
}

pub fn cache_usage() -> usize {
    get_string_cache().usage()
}

pub fn cache_clear() {
    get_string_cache().clear();
}

/// Create a cached Python `str` from a string slice.
#[inline]
pub fn cached_py_string<'py>(py: Python<'py>, s: &str) -> Bound<'py, PyString> {
    // from tests, 0 and 1 character strings are faster not cached
    if (2..64).contains(&s.len()) {
        get_string_cache().get_or_insert(py, s)
    } else {
        Bound::<PyString>::new(py, s)
    }
}

// capacity should be a power of 2
const CAPACITY: usize = 16_384;
type Entry = Option<(u64, String)>;

/// Fully associative cache with LRU replacement policy.
struct PyStringCache {
    entries: Box<[Entry; CAPACITY]>,
    hash_builder: RandomState,
}

const ARRAY_REPEAT_VALUE: Entry = None;

impl Default for PyStringCache {
    fn default() -> Self {
        Self {
            #[allow(clippy::large_stack_arrays)]
            entries: Box::new([ARRAY_REPEAT_VALUE; CAPACITY]),
            hash_builder: RandomState::default(),
        }
    }
}

impl PyStringCache {
    fn get_or_insert<'py>(&mut self, py: Python<'py>, s: &str) -> Bound<'py, PyString> {
        let hash = self.hash_builder.hash_one(s);
        let hash_index = hash as usize % CAPACITY;

        // We try up to 5 contiguous slots to find a match or an empty slot
        for index in hash_index..hash_index.wrapping_add(5) {
            if let Some(entry) = self.entries.get_mut(index) {
                if let Some((entry_hash, cached_str)) = entry {
                    if *entry_hash == hash && cached_str == s {
                        return Bound::<PyString>::new(py, cached_str);
                    }
                } else {
                    // Empty slot — use it
                    *entry = Some((hash, s.to_owned()));
                    return Bound::<PyString>::new(py, s);
                }
            } else {
                break;
            }
        }
        // All 5 slots full, replace the first (LRU)
        if let Some(entry) = self.entries.get_mut(hash_index) {
            *entry = Some((hash, s.to_owned()));
        }
        Bound::<PyString>::new(py, s)
    }

    fn usage(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    fn clear(&mut self) {
        self.entries.fill_with(|| None);
    }
}
