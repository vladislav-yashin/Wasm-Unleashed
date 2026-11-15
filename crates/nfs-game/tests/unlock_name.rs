//! The browser shell may name the port-created profile before the save directory is read
//! (`crates/open-unleashed/src/web.rs`). The name is the only marker the `.sav` format has, so the
//! configured name must create, recognize and display that profile.
use nfs_game::career::CareerTables;
use nfs_game::save::{ensure_unlock_all_profile, load_profile_career, set_unlock_all_name};

fn tables() -> CareerTables {
    CareerTables { raw: Default::default(), tracks: vec![], tournaments: vec![], parts: vec![], personalities: vec![], missions: vec![], strings: Default::default(), catalog: vec![], package_count: 0 }
}

#[test]
fn a_configured_name_creates_and_recognizes_the_unlock_all_profile() {
    assert!(!set_unlock_all_name("a/b"), "an invalid name leaves the default in place");
    assert!(set_unlock_all_name("Nick"), "the first valid name wins");
    assert!(set_unlock_all_name("Nick"), "repeating it is harmless");
    assert!(!set_unlock_all_name("Other"), "a later name cannot move a running profile");
    let dir = std::env::temp_dir().join(format!("nfs-unlock-name-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let tables = tables();
    let p = ensure_unlock_all_profile(&dir, &tables).unwrap().expect("a fresh directory creates the profile");
    assert_eq!(p.stem(), "0Nick");
    assert_eq!(p.display_name(), "Nick");
    assert!(!dir.join("0UnlockAll.sav").exists(), "the default name is not created alongside it");
    let (career, _) = load_profile_career(&dir, &p, &tables).unwrap();
    assert!(career.unlock_all, "the configured name marks the profile");
    assert_eq!(ensure_unlock_all_profile(&dir, &tables).unwrap(), None, "one file is enough");
    let _ = std::fs::remove_dir_all(&dir);
}
