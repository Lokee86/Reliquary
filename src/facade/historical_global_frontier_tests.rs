use super::frontier_end;
use crate::Container;
use std::path::PathBuf;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("global-frontier-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> PathBuf {
        self.0.join("owner.rel")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn file_end(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).unwrap().len()
}

#[test]
fn clock_neutral_frames_follow_the_existing_post_observation_global_frontier() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut container = Container::create(&path).unwrap();
    container.append(b"clock-neutral-before-global").unwrap();
    let before_any_global = file_end(&path);
    assert_eq!(container.allocate_version().unwrap(), 1);
    container.append(b"clock-neutral-at-global-one").unwrap();
    let after_first_global = file_end(&path);
    assert_eq!(container.allocate_version().unwrap(), 2);
    container.append(b"clock-neutral-at-global-two").unwrap();
    let after_second_global = file_end(&path);
    container.sync().unwrap();

    assert_eq!(frontier_end(&container, 0).unwrap(), before_any_global);
    assert_eq!(frontier_end(&container, 1).unwrap(), after_first_global);
    assert_eq!(frontier_end(&container, 2).unwrap(), after_second_global);
    assert!(frontier_end(&container, 3).is_err());
}

#[test]
fn embedded_version_range_is_atomic_at_its_physical_frame_frontier() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut container = Container::create(&path).unwrap();

    // Minimal version-range header consumed by the same observer used during
    // Container replay. The payload represents a three-version completion
    // range in one physical frame; it is deliberately not a fake CVAVERS tag.
    let mut completion = vec![0_u8; 84];
    completion[..8].copy_from_slice(b"CVAINSC7");
    completion[64..72].copy_from_slice(&1_u64.to_le_bytes());
    completion[72..76].copy_from_slice(&3_u32.to_le_bytes());
    completion[76..84].copy_from_slice(&100_i64.to_le_bytes());
    container.append(&completion).unwrap();
    let range_end = file_end(&path);
    container.append(b"clock-neutral-after-range").unwrap();
    let following_clock_neutral_end = file_end(&path);
    container.sync().unwrap();
    drop(container);

    // Reopen observes embedded ranges using Container's real version observer.
    let reopened = Container::open(&path).unwrap();
    assert_eq!(reopened.latest_version(), 3);
    assert_eq!(frontier_end(&reopened, 1).unwrap(), reopened.header_len);
    assert_eq!(frontier_end(&reopened, 2).unwrap(), reopened.header_len);
    assert_eq!(
        frontier_end(&reopened, 3).unwrap(),
        following_clock_neutral_end
    );
    assert!(range_end < following_clock_neutral_end);
}
