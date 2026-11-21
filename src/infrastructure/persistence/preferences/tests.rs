use super::*;
#[test]
fn settings_roundtrip_survives_atomic_replacement_and_rejects_invalid_files() {
    let path = std::env::temp_dir().join(format!("vjay-preferences-{}.json", std::process::id()));
    let store = JsonSettingsStore(path.clone());
    let mut p = Preferences::default();
    store.save(&p).unwrap();
    p.default_project_name = "Camera set.vjay.json".into();
    p.audio.channel_pair = 2;
    p.video.fullscreen = true;
    store.save(&p).unwrap();
    assert_eq!(store.load().unwrap(), p);
    std::fs::write(&path, b"{\"video\":{\"frame_rate\":0}}").unwrap();
    assert!(store.load().is_err());
    std::fs::write(&path, vec![0; 65537]).unwrap();
    assert!(store.load().unwrap_err().contains("64 KiB"));
    std::fs::remove_file(path).unwrap();
    assert_eq!(store.load().unwrap(), Preferences::default());
}
