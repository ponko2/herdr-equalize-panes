use anyhow::{Context, Result};
use std::{
    fs::{self, File},
    path::Path,
};

const LOCK_FILE: &str = "equalize-panes.lock";

pub struct StateLock {
    _file: File,
}

impl StateLock {
    pub fn acquire(state_dir: &Path) -> Result<Self> {
        fs::create_dir_all(state_dir)
            .with_context(|| format!("creating {}", state_dir.display()))?;

        let path = state_dir.join(LOCK_FILE);
        let file = File::options()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;

        file.lock()
            .with_context(|| format!("locking {}", path.display()))?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        process,
        sync::{
            atomic::{AtomicU32, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };

    fn unique_state_dir() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let name = format!(
            "eqp-lock-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        std::env::temp_dir().join(name)
    }

    fn is_free(state_dir: &Path) -> bool {
        let file = File::options()
            .create(true)
            .write(true)
            .truncate(false)
            .open(state_dir.join(LOCK_FILE))
            .expect("the lock file is there");
        file.try_lock().is_ok()
    }

    #[test]
    fn a_lock_lasts_only_as_long_as_the_handle() {
        let state_dir = unique_state_dir();

        let lock = StateLock::acquire(&state_dir).unwrap();
        assert!(!is_free(&state_dir), "the lock is held");

        drop(lock);
        assert!(is_free(&state_dir), "dropping the handle released it");

        fs::remove_dir_all(&state_dir).expect("the test made this directory");
    }

    #[test]
    fn acquiring_waits_for_a_competing_hook_instead_of_giving_up() {
        let state_dir = unique_state_dir();
        let held = StateLock::acquire(&state_dir).unwrap();

        let (acquired, waiting) = mpsc::channel();
        let competitor = thread::spawn({
            let state_dir = state_dir.clone();
            move || {
                let lock = StateLock::acquire(&state_dir).unwrap();
                acquired.send(()).expect("the test is still listening");
                lock
            }
        });

        assert!(
            waiting.recv_timeout(Duration::from_millis(100)).is_err(),
            "the second acquire should still be waiting"
        );

        drop(held);
        waiting
            .recv_timeout(Duration::from_secs(5))
            .expect("releasing the lock should let the waiting one through");
        let taken_over = competitor.join().expect("no panic in the waiting thread");
        drop(taken_over);

        fs::remove_dir_all(&state_dir).expect("the test made this directory");
    }

    #[test]
    fn acquiring_creates_the_state_directory() {
        let state_dir = unique_state_dir().join("nested");
        let _lock = StateLock::acquire(&state_dir).unwrap();

        assert!(state_dir.is_dir());
        fs::remove_dir_all(state_dir.parent().unwrap()).expect("the test made this directory");
    }
}
