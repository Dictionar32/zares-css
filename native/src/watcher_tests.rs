//! Test-only module extracted from watcher.rs

#[cfg(test)]
mod tests {
    use crate::watcher::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn event_kind_as_str() {
        assert_eq!(WatchEventKind::Add.as_str(), "add");
        assert_eq!(WatchEventKind::Change.as_str(), "change");
        assert_eq!(WatchEventKind::Remove.as_str(), "unlink");
        assert_eq!(WatchEventKind::Rename.as_str(), "rename");
    }

    #[test]
    fn watcher_starts_on_real_dir() {
        let events = Arc::new(Mutex::new(Vec::<WatchEvent>::new()));
        let events_clone = Arc::clone(&events);

        let tmp_dir = tempfile::TempDir::new().expect("create temp dir");
        let watch_path = tmp_dir.path().to_str().expect("valid temp path");

        let handle = start_watch(watch_path, move |ev| {
            events_clone.lock().unwrap().push(ev);
        });

        assert!(handle.is_ok(), "watcher should start on {}", watch_path);
        // handle drop stops the watcher
    }
}
