use super::*;

#[test]
#[ignore = "requires installed Japanese fonts"]
fn italic_and_oblique_keep_japanese_titles_readable() {
    let mut system = FontSystem::new();
    for style in [Style::Italic, Style::Oblique] {
        let attrs = Attrs::new()
            .family(Family::Serif)
            .weight(Weight::BOLD)
            .style(style);
        let mut buffer = Buffer::new(&mut system, Metrics::new(48.0, 60.0));
        buffer.set_size(&mut system, Some(960.0), Some(540.0));
        buffer.set_text(
            &mut system,
            "曲名 / アルバム Album",
            &attrs,
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut system, false);
        resolve_missing_style(&mut buffer, &mut system, &attrs);
        let glyphs: Vec<_> = buffer.layout_runs().flat_map(|r| r.glyphs).collect();
        assert!(glyphs.len() > 8);
        assert!(
            glyphs.iter().all(|g| g.glyph_id != 0),
            "{style:?} must retain Japanese glyphs"
        );
    }
}
