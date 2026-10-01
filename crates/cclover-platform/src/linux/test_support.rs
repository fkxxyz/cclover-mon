use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

pub(super) struct Fixture {
    root: PathBuf,
}

impl Fixture {
    pub(super) fn new(name: &str) -> Self {
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cclover-mon-{name}-{}-{timestamp}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create fixture root");
        Self { root }
    }

    pub(super) fn path(&self) -> &Path {
        &self.root
    }

    pub(super) fn dir(&self, relative: impl AsRef<Path>) {
        fs::create_dir_all(self.root.join(relative)).expect("create fixture directory");
    }

    pub(super) fn write(&self, relative: impl AsRef<Path>, contents: impl AsRef<[u8]>) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create fixture file parent");
        }
        fs::write(path, contents).expect("write fixture file");
    }

    pub(super) fn symlink_to(
        &self,
        target_relative: impl AsRef<Path>,
        link_relative: impl AsRef<Path>,
    ) {
        let link = self.root.join(link_relative);
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent).expect("create fixture symlink parent");
        }
        symlink(self.root.join(target_relative), link).expect("create fixture symlink");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
