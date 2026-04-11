use std::hash::Hash;

use indexmap::{IndexMap, IndexSet};

const OVERLOAD_ALERT_RATIO: f32 = 0.8;

/// utility for stream loading
pub struct SaveStream<T> {
    ring: IndexSet<T>,
    frnt: usize,
    caps: usize,
}

impl<T: Copy + Eq + Hash> SaveStream<T> {
    pub fn new(caps: usize) -> Self {
        SaveStream {
            ring: IndexSet::with_capacity(caps),
            frnt: 0,
            caps,
        }
    }

    pub fn load(&mut self, keys: &[T], mut callback: impl FnMut(T, bool)) {
        // when loading requests' number is too close to the cap, ignore the whole load
        if keys.len() as f32 >= self.caps as f32 * OVERLOAD_ALERT_RATIO {
            return;
        }

        for &key in keys {
            if self.frnt >= self.ring.len() {
                // Out of bounds, just insert
                if self.ring.insert(key) {
                    callback(key, true);
                }

                self.frnt = self.ring.len() % self.caps;
                continue;
            }

            let Ok(replaced) = self.ring.replace_index(self.frnt, key) else {
                // already loaded skipped
                continue;
            };

            // move forward
            self.frnt = (self.frnt + 1) % self.caps;
            callback(replaced, false);
            callback(key, true);
        }
    }

    pub fn load_filtered(&mut self, keys: &[T]) -> IndexMap<T, bool> {
        let mut buf = IndexMap::new();
        self.load(&keys, |key, loaded| match buf.get(&key) {
            None => {
                buf.insert(key, loaded);
            }
            Some(true) => {
                assert!(!loaded);
                buf.swap_remove(&key);
            }
            Some(false) => {
                assert!(loaded);
                buf.swap_remove(&key);
            }
        });

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_stream_load() {
        let mut stream = SaveStream::new(5);

        // Initial load
        let seq = stream.load_filtered(&[1, 2, 3]);
        assert_eq!(seq.get(&1), Some(&true));
        assert_eq!(seq.get(&2), Some(&true));
        assert_eq!(seq.get(&3), Some(&true));

        // Load with some overlap, no unnecessary loads
        let seq = stream.load_filtered(&[3, 4, 5]);
        assert_eq!(seq.get(&4), Some(&true));
        assert_eq!(seq.get(&5), Some(&true));

        // Load more, unloads oldest ones
        let seq = stream.load_filtered(&[6, 7, 8]);
        assert_eq!(seq.get(&1), Some(&false));
        assert_eq!(seq.get(&2), Some(&false));
        assert_eq!(seq.get(&3), Some(&false));
        assert_eq!(seq.get(&6), Some(&true));
        assert_eq!(seq.get(&7), Some(&true));
        assert_eq!(seq.get(&8), Some(&true));

        // Load exceeding capacity, should ignore
        let seq = stream.load_filtered(&[6, 7, 8, 9, 10]);
        assert!(seq.is_empty());

        // Load with all keys already present
        let seq = stream.load_filtered(&[6, 7, 8]);
        assert!(seq.is_empty());
    }
}
