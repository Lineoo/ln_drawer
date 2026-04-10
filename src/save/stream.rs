use std::hash::Hash;

use indexmap::IndexSet;

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

    pub fn load(&mut self, keys: &[T], mut load: impl FnMut(T), mut unload: impl FnMut(T)) {
        // when loading requests' number is too close to the cap, ignore the whole load
        if keys.len() as f32 >= self.caps as f32 * OVERLOAD_ALERT_RATIO {
            return;
        }

        for &key in keys {
            if self.frnt >= self.ring.len() {
                // Out of bounds, just insert
                if self.ring.insert(key) {
                    load(key);
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

            if key != replaced {
                load(key);
                unload(replaced);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_stream_load() {
        let mut stream = SaveStream::new(5);

        let mut loaded = Vec::new();
        let mut unloaded = Vec::new();

        // Initial load
        let load = |key| loaded.push(key);
        let unload = |key| unloaded.push(key);
        stream.load(&[1, 2, 3], load, unload);
        assert_eq!(loaded, vec![1, 2, 3]);
        assert!(unloaded.is_empty());

        // Load with some overlap, no unnecessary loads
        loaded.clear();
        let load = |key| loaded.push(key);
        let unload = |key| unloaded.push(key);
        stream.load(&[3, 4, 5], load, unload);
        assert_eq!(loaded, vec![4, 5]);
        assert!(unloaded.is_empty());

        // Load more, unloads oldest ones
        loaded.clear();
        let load = |key| loaded.push(key);
        let unload = |key| unloaded.push(key);
        stream.load(&[6, 7, 8], load, unload);
        assert_eq!(loaded, vec![6, 7, 8]);
        assert_eq!(unloaded, vec![1, 2, 3]);

        // Load exceeding capacity, should ignore
        loaded.clear();
        unloaded.clear();
        let load = |key| loaded.push(key);
        let unload = |key| unloaded.push(key);
        stream.load(&[6, 7, 8, 9, 10], load, unload);
        assert!(loaded.is_empty());
        assert!(unloaded.is_empty());

        // Load with all keys already present
        loaded.clear();
        unloaded.clear();
        let load = |key| loaded.push(key);
        let unload = |key| unloaded.push(key);
        stream.load(&[6, 7, 8], load, unload);
        assert!(loaded.is_empty());
        assert!(unloaded.is_empty());
    }
}
