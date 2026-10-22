use super::*;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("reclamation-fence-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn grow(path: &Path) {
    let mut file = OpenOptions::new().append(true).open(path).unwrap();
    file.write_all(&[1, 2, 3, 4]).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn reclamation_source_rejects_growth_before_policy_scan_without_creating_output() {
    let scratch = Scratch::new();
    let input = scratch.0.join("source.rel");
    let output = scratch.0.join("output.rel");
    let mut owner = crate::Cva::create(&input).unwrap();
    let before = fs::read(&input).unwrap();
    let source = ReliquaryReclamationSource::prepare(&mut owner.container).unwrap();
    assert_eq!(fs::read(&input).unwrap(), before);
    grow(&input);
    let error = crate::maintenance::reclamation::reclaim_reliquary(source, &output)
        .err()
        .unwrap();
    assert_eq!(error, crate::ContainerError::UnstableSnapshot.to_string());
    assert!(!output.exists());
    assert_eq!(&fs::read(&input).unwrap()[..before.len()], before);
}

#[test]
fn reclamation_source_rejects_growth_after_policy_read_before_copy() {
    let scratch = Scratch::new();
    let input = scratch.0.join("source.phy");
    let output = scratch.0.join("output.phy");
    let mut owner = crate::Phylactery::create(&input).unwrap();
    let before = fs::read(&input).unwrap();
    let source = PhylacteryReclamationSource::prepare(&mut owner.container).unwrap();
    {
        let snapshot = source.history_source().unwrap();
        for object in source.objects() {
            snapshot.read(*object).unwrap();
        }
    }
    assert_eq!(fs::read(&input).unwrap(), before);
    grow(&input);
    let error = source.copy(&HashSet::new(), &output).err().unwrap();
    assert_eq!(error, crate::ContainerError::UnstableSnapshot.to_string());
    assert!(!output.exists());
    assert_eq!(&fs::read(&input).unwrap()[..before.len()], before);
}
