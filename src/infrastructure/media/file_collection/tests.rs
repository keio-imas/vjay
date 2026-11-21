use super::*;
fn request(path: &Path, index: usize) -> AssetRequest {
    AssetRequest {
        provider: "video-collection",
        selection_parameter: Some(6),
        file_output: false,
        location: path.to_string_lossy().into_owned(),
        options: std::collections::BTreeMap::from([("index".into(), index.to_string())]),
    }
}
#[test]
fn folders_sort_and_playlist_order_and_original_files_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["b.MOV", "a.mp4", "曲名.webm", "ignore.png"] {
        std::fs::write(dir.path().join(name), name).unwrap();
    }
    let factory = FileCollectionFactory::default();
    let mut job = factory.start(&request(dir.path(), 1)).unwrap();
    let asset = job.poll(FrameDelivery::Exact).unwrap().unwrap();
    assert_eq!(asset.path().file_name().unwrap(), "b.MOV");
    assert_eq!(asset.items().len(), 3);
    let list = dir.path().join("set.m3u8");
    let content = "\u{feff}#EXTM3U\n#EXTINF:1,Music\n曲名.webm\nb.MOV\na.mp4\n";
    std::fs::write(&list, content).unwrap();
    let mut job = factory.start(&request(&list, 0)).unwrap();
    let asset = job.poll(FrameDelivery::Exact).unwrap().unwrap();
    assert_eq!(asset.path().file_name().unwrap(), "曲名.webm");
    assert_eq!(asset.items()[2].label, "a.mp4");
    assert_eq!(std::fs::read_to_string(&list).unwrap(), content);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b.MOV")).unwrap(),
        "b.MOV"
    );
    let mut job = factory.start(&request(&list, 3)).unwrap();
    assert!(matches!(job.poll(FrameDelivery::Exact).unwrap(),Err(e) if e.contains("index")));
}
