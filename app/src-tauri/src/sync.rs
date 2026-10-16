use std::sync::{Mutex, MutexGuard, PoisonError};

// ほかのスレッドが panic してロックが壊れても、中身はそのまま使える。unwrap で連鎖して落ちないようにする
pub trait LockExt<T> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
