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
            unload(replaced);
        }
    }
}
