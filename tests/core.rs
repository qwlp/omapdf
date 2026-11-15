use omapdf::{
    document::{Document, parse_bbox},
    state::{History, Library, Location},
};

#[test]
fn bbox_unicode_search_and_page_geometry() {
    let pages = parse_bbox(r#"<!DOCTYPE html><html><body><doc><page width="612" height="792"><word xMin="12" yMin="24" xMax="42" yMax="36">Über</word><word xMin="44" yMin="24" xMax="80" yMax="36">Research</word></page><page width="800" height="600"/></doc></body></html>"#).unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].words[0].rect.width, 30.);
    assert_eq!(pages[0].word_at(20., 30.), Some(0));
    assert_eq!(pages[0].word_at(60., 30.), Some(1));
    assert_eq!(pages[0].word_at(200., 200.), None);
    assert_eq!(pages[0].find("ÜBER research"), vec![(0, 2)]);
    assert!(pages[0].find("").is_empty());
    assert_eq!(
        pages[0].find_regex(&regex::Regex::new(r"(?i)über\s+res\w+").unwrap()),
        vec![(0, 2)]
    );
    assert!(
        pages[0]
            .find_regex(&regex::Regex::new("^").unwrap())
            .is_empty()
    );
    assert!(pages[1].words.is_empty());
    assert!(parse_bbox("<doc><page width=\"0\" height=\"5\"/></doc>").is_err());
}

#[test]
fn original_key_syntax_sequences_modifiers_and_overrides() {
    use omapdf::config::{Binding, Config, parse_sequence};
    assert_eq!(
        parse_sequence("g<C-n>Dt").unwrap(),
        ["g", "ctrl-n", "D", "t"]
    );
    assert_eq!(parse_sequence("<C-<pagedown>>").unwrap(), ["ctrl-pagedown"]);
    assert_eq!(parse_sequence("<S-<space>>").unwrap(), ["shift- "]);
    assert!(parse_sequence("<C-<pageup>").is_err());
    let mut config = Config::default();
    assert_eq!(config.binding(&["g".into()]), Binding::Prefix);
    config.parse_keys("next_page J\nzoom_in J\n").unwrap();
    assert_eq!(config.binding(&["J".into()]), Binding::Exact("zoom_in"));
    config.parse_keys("zoom_in <unbound>").unwrap();
    assert_eq!(config.binding(&["J".into()]), Binding::Missing);
}

#[test]
fn omapdf_preferences_roundtrip_preserves_unknown_options_and_comments() {
    use omapdf::config::Config;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prefs_user.config");
    let original = "# Keep this comment\ninverse_search_command editor --line %2 %1\nsearch_url_g https://example.test/#fragment\ndark_mode 1\nmove_amount 120\nzoom_inc_factor 1.3\nzoom_inc_factor 1.4\n";
    std::fs::write(&path, original).unwrap();
    let mut config = Config::load(directory.path()).unwrap();
    assert!(config.dark);
    assert_eq!(config.move_amount, 120.);
    assert_eq!(config.value("vertical_move_amount"), "2");
    assert_eq!(config.zoom_factor, 1.4);
    config
        .set_preferences(&[
            ("default_dark_mode".into(), "0".into()),
            ("vertical_move_amount".into(), "1.5".into()),
            ("zoom_inc_factor".into(), "1.25".into()),
        ])
        .unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("# Keep this comment"));
    assert!(saved.contains("inverse_search_command editor --line %2 %1"));
    assert!(saved.contains("https://example.test/#fragment"));
    assert_eq!(
        saved
            .lines()
            .filter(|line| line.starts_with("zoom_inc_factor "))
            .count(),
        1
    );
    let restored = Config::load(directory.path()).unwrap();
    assert!(!restored.dark);
    assert_eq!(restored.move_amount, 90.);
    assert_eq!(restored.zoom_factor, 1.25);
    let before = std::fs::read(&path).unwrap();
    assert!(config.set_preference("render_scale", "NaN").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    config
        .set_preference("highlight_color_z", "0.2 0.4 0.6")
        .unwrap();
    assert_eq!(
        Config::load(directory.path()).unwrap().highlight_colors[25],
        0x33669966
    );
}

#[test]
fn omarchy_palette_uses_named_theme_colors_and_ignores_invalid_values() {
    use omapdf::appearance::Appearance;
    let mut theme = Appearance::default();
    theme.apply_colors("background = \"#060B1E\"\ndark_background = \"#040816\"\nforeground = \"#ffcead\"\naccent = \"#7d82d9\"\nlighter_background = \"#131a3a\"\nmuted = \"not-a-color\"\n");
    assert_eq!(theme.panel, 0x060b1e);
    assert_eq!(theme.background, 0x040816);
    assert_eq!(theme.foreground, 0xffcead);
    assert_eq!(theme.accent, 0x7d82d9);
    assert_eq!(theme.border, 0x131a3a);
    assert_eq!(theme.muted, Appearance::default().muted);
}

#[test]
fn outline_and_links_from_poppler_metadata() {
    use omapdf::document::parse_metadata;
    let mut pages = parse_bbox(
        r#"<doc><page width="612" height="792"/><page width="612" height="792"/></doc>"#,
    )
    .unwrap();
    let outline = parse_metadata(r##"<pdf2xml><page number="1"><text left="10" top="20" width="30" height="12"><a href="#2">Chapter</a></text></page><outline><item page="1">Introduction<outline><item page="2">Details</item></outline></item></outline></pdf2xml>"##, &mut pages).unwrap();
    assert_eq!(pages[0].links[0].target, "#2");
    assert_eq!(outline[0].title, "Introduction");
    assert_eq!(outline[1].depth, 1);
    assert_eq!(outline[1].page, 1);
}

#[test]
fn unicode_input_composition_ranges_and_editing() {
    use omapdf::text::TextBuffer;
    let mut input = TextBuffer::default();
    input.replace(None, "A🦀ไทย", false);
    assert_eq!(input.to_utf16(0..input.text.len()), 0..6);
    input.replace(Some(1..3), "研究", true);
    assert_eq!(input.text, "A研究ไทย");
    input.replace(None, "漢字", false);
    assert_eq!(input.text, "A漢字ไทย");
    assert!(input.marked.is_none());
    input.backspace();
    assert_eq!(input.text, "A漢ไทย");
    input.delete();
    assert_eq!(input.text, "A漢ทย");
    input.selection = 0..input.text.len();
    input.replace(None, "a\nb", false);
    assert_eq!(input.text, "a b");
}

#[test]
fn single_instance_forwards_and_recovers_socket() {
    use omapdf::ipc::{Instance, Request, Server};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.json");
    let initial = Request::default();
    let Instance::Primary(mut primary) = Server::start(&path, &initial).unwrap() else {
        panic!("expected primary");
    };
    let receiver = primary.take_receiver();
    let request = Request {
        commands: vec!["goto 3".into()],
        ..Default::default()
    };
    assert!(matches!(
        Server::start(&path, &request).unwrap(),
        Instance::Forwarded
    ));
    assert_eq!(
        receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .commands,
        ["goto 3"]
    );
    drop(primary);
    assert!(!path.with_extension("sock").exists());
    assert!(matches!(
        Server::start(&path, &initial).unwrap(),
        Instance::Primary(_)
    ));
}

#[test]
fn atomic_library_roundtrip_and_corruption_detection() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested/library.json");
    let mut library = Library::load(&path).unwrap();
    library.touch("abc", std::path::Path::new("/tmp/研究.pdf"));
    library.touch("def", std::path::Path::new("/tmp/second.pdf"));
    library.touch("abc", std::path::Path::new("/tmp/研究.pdf"));
    library.documents.get_mut("abc").unwrap().page = 12;
    library.save(&path).unwrap();
    let restored = Library::load(&path).unwrap();
    assert_eq!(restored.recent, ["abc", "def"]);
    assert_eq!(restored.documents["abc"].page, 12);
    std::fs::write(&path, "broken").unwrap();
    assert!(Library::load(&path).is_err());
}

#[test]
fn branching_history_discards_forward_branch() {
    let at = |page| Location {
        document: "doc".into(),
        page,
        zoom: 1.,
        ..Default::default()
    };
    let mut history = History::default();
    history.push(at(0));
    history.push(at(3));
    assert_eq!(history.back(at(9)), Some(at(3)));
    assert_eq!(history.forward(at(3)), Some(at(9)));
    assert_eq!(history.back(at(9)), Some(at(3)));
    history.push(at(3));
    assert_eq!(history.forward(at(4)), None);
}

/// Exercises native MuPDF, including paths with spaces and shell metacharacters.
#[test]
fn real_pdf_open_search_render_and_bad_input() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("paper $(false) ' spaced.pdf");
    write_pdf(&path);
    let doc = Document::open(&path).unwrap();
    assert_eq!(doc.pages.len(), 1);
    assert_eq!(doc.pages[0].find("hello research"), vec![(0, 2)]);
    let png = doc.render(0, 400, directory.path(), false).unwrap();
    assert_eq!(&std::fs::read(png).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    assert!(doc.render(1, 400, directory.path(), false).is_err());
    let rasterizer = omapdf::raster::Rasterizer::new();
    // Exercise optional filtering and its cache independently of native defaults.
    rasterizer.set_quality(2.);
    rasterizer.set_sharpness(0.35);
    let doc = std::sync::Arc::new(doc);
    let native = rasterizer.render(doc.clone(), 0, 600, false).unwrap();
    assert_eq!(native.size(0).width.0, 600);
    assert_eq!(native.size(0).height.0, 400);
    let pixels = native.as_bytes(0).unwrap();
    assert!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255)
    );
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[0] < 128));
    assert!(std::sync::Arc::ptr_eq(
        &native,
        &rasterizer.render(doc.clone(), 0, 600, false).unwrap()
    ));
    rasterizer.set_quality(1.);
    let draft = rasterizer.render(doc.clone(), 0, 600, false).unwrap();
    assert_eq!(draft.size(0), native.size(0));
    assert!(!std::sync::Arc::ptr_eq(&draft, &native));
    assert_ne!(draft.as_bytes(0), native.as_bytes(0));
    rasterizer.set_quality(2.);
    assert!(std::sync::Arc::ptr_eq(
        &native,
        &rasterizer.render(doc.clone(), 0, 600, false).unwrap()
    ));
    rasterizer.set_sharpness(0.);
    let soft = rasterizer.render(doc.clone(), 0, 600, false).unwrap();
    assert_ne!(soft.as_bytes(0), native.as_bytes(0));
    rasterizer.set_sharpness(0.35);
    assert!(std::sync::Arc::ptr_eq(
        &native,
        &rasterizer.render(doc.clone(), 0, 600, false).unwrap()
    ));
    let dark = rasterizer.render(doc.clone(), 0, 600, true).unwrap();
    for (light, dark) in pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(dark.as_bytes(0).unwrap().as_chunks::<4>().0.iter())
    {
        assert_eq!(light[0], 255 - dark[0]);
        assert_eq!(light[1], 255 - dark[1]);
        assert_eq!(light[2], 255 - dark[2]);
    }
    rasterizer.set_contrast(0.5);
    let dim = rasterizer.render(doc.clone(), 0, 600, true).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&dim, &dark));
    for (full, dimmed) in dark
        .as_bytes(0)
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(dim.as_bytes(0).unwrap().as_chunks::<4>().0)
    {
        assert_eq!(dimmed[0], (full[0] as u16 * 128 / 255) as u8);
        assert_eq!(dimmed[1], (full[1] as u16 * 128 / 255) as u8);
        assert_eq!(dimmed[2], (full[2] as u16 * 128 / 255) as u8);
    }
    assert!(rasterizer.render(doc.clone(), 1, 600, false).is_err());
    assert!(rasterizer.render(doc.clone(), 0, 0, false).is_err());
    // Invalid work must not terminate the persistent renderer.
    assert!(rasterizer.render(doc.clone(), 0, 640, false).is_ok());
    rasterizer.shutdown();
    rasterizer.shutdown();
    assert!(rasterizer.render(doc, 0, 700, false).is_err());
    std::fs::write(&path, "This is not a PDF").unwrap();
    assert!(Document::open(&path).is_err());
}

fn write_pdf(path: &std::path::Path) {
    write_pdf_pages(path, false);
}
fn write_pdf_pages(path: &std::path::Path, two_pages: bool) {
    write_pdf_stream(
        path,
        two_pages,
        "BT /F1 18 Tf 30 150 Td (Hello research world) Tj ET\n",
    );
}
fn write_pdf_stream(path: &std::path::Path, two_pages: bool, stream: &str) {
    write_pdf_stream_geometry(path, two_pages, stream, "/MediaBox [0 0 300 200]");
}
fn write_pdf_stream_geometry(
    path: &std::path::Path,
    two_pages: bool,
    stream: &str,
    geometry: &str,
) {
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        format!(
            "<< /Type /Page /Parent 2 0 R {geometry} /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>"
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ),
    ];
    if two_pages {
        objects[1] = "<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>".into();
        objects.push(objects[2].clone());
    }
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = vec![0];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
    }
    let xref = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()));
    for offset in &offsets[1..] {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        offsets.len()
    ));
    std::fs::write(path, pdf).unwrap();
}

#[test]
fn epub_preview_defers_text_but_preserves_search_and_reflow_ids() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/reading-lab.epub");
    let preview = Document::open_preview(&path, Default::default()).unwrap();
    assert!(preview.reflowable);
    assert_eq!(preview.indexed_pages(), 1);
    let snapshot = preview.clone();
    let full = Document::open(&path).unwrap();
    assert_eq!(preview.id, full.id);
    assert_eq!(preview.pages.len(), full.pages.len());
    let target = full.find_text_near("Compilation", 0.1).unwrap();
    assert!(target.0 > 0);
    preview.ensure_page_text(target.0).unwrap();
    assert!(snapshot.text_ready(target.0));
    assert_eq!(preview.pages[target.0].text(), full.pages[target.0].text());
    let matches = omapdf::search::find(&preview, "Compilation", Default::default()).unwrap();
    assert!(!matches.is_empty());
    assert_eq!(snapshot.indexed_pages(), snapshot.pages.len());
}

#[test]
fn original_epub_fixture_has_native_text_links_contents_and_reflow_anchors() {
    use omapdf::{document::EpubOptions, state::Location};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/reading-lab.epub");
    let doc = Document::open(&path).unwrap();
    assert!(doc.reflowable);
    assert!(doc.title.contains("Omapdf Reading Lab"));
    assert!(doc.author.contains("Omapdf contributors"));
    assert!(doc.pages.len() > 100);
    assert!(doc.outline.len() > 20);
    assert!(doc.pages.iter().any(|page| {
        page.links
            .iter()
            .any(|link| link.target.starts_with('#') && link.target.contains('@'))
    }));
    assert!(doc.pages.iter().any(|page| {
        page.links
            .iter()
            .any(|link| link.target.starts_with("https://"))
    }));
    let hit = doc.find_text_near("Compilation", 0.1).unwrap();
    let old = Location {
        document: doc.id.clone(),
        page: hit.0,
        offset: [
            0.,
            -doc.pages[hit.0].words[hit.1].rect.y / doc.pages[hit.0].width,
        ],
        ..Default::default()
    };
    let reflowed = Document::open_with_options(
        &path,
        EpubOptions {
            font_size: 17.,
            ..Default::default()
        },
    )
    .unwrap();
    assert_ne!(reflowed.id, doc.id);
    assert!(reflowed.pages.len() > doc.pages.len());
    let mapped = reflowed.remap_location(&doc, &old);
    assert_eq!(mapped.document, reflowed.id);
    assert!(
        reflowed.pages[mapped.page]
            .text()
            .to_lowercase()
            .contains("compilation")
    );
    let renderer = omapdf::raster::Rasterizer::new();
    renderer.set_quality(1.);
    let output = renderer
        .render(std::sync::Arc::new(doc), hit.0, 420, false)
        .unwrap();
    assert_eq!(output.size(0).width.0, 420);
    assert!(
        output
            .as_bytes(0)
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[0] < 128)
    );
    assert!(
        EpubOptions {
            font_size: f32::NAN,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn preview_shares_lazy_text_and_search_indexes_unvisited_pages() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lazy.pdf");
    write_pdf_pages(&path, true);
    let doc = Document::open_preview(&path, Default::default()).unwrap();
    let snapshot = doc.clone();
    assert_eq!(doc.pages.len(), 2);
    assert_eq!(doc.indexed_pages(), 1);
    assert!(doc.pages[1].words.is_empty());
    doc.ensure_all_text_until(&std::sync::atomic::AtomicBool::new(true))
        .unwrap();
    assert_eq!(doc.indexed_pages(), 1);
    let hits = omapdf::search::find(&doc, "research", Default::default()).unwrap();
    assert_eq!(hits.len(), 2);
    assert!(snapshot.pages[1].words.ready());
    assert_eq!(snapshot.pages[1].words[1].text, "research");
    assert_eq!(doc.id, Document::open(&path).unwrap().id);
    assert!(doc.ensure_page_text(10).is_err());
}

#[test]
fn document_window_preference_validates_and_persists() {
    use omapdf::config::Config;
    let mut config = Config::default();
    assert_eq!(config.value("open_in_new_windows"), "0");
    config.apply_preference("open_in_new_windows", "1").unwrap();
    assert_eq!(config.value("open_in_new_windows"), "1");
    assert!(
        config
            .apply_preference("open_in_new_windows", "tabs")
            .is_err()
    );
}

#[test]
fn concurrent_readers_preserve_documents_marks_and_deletions() {
    use omapdf::state::merge_library;
    let mut baseline = Library::default();
    baseline.touch("a", std::path::Path::new("a.pdf"));
    baseline.touch("b", std::path::Path::new("b.pdf"));
    baseline.global_marks.insert('A', Location::default());
    let mut first = baseline.clone();
    first.documents.get_mut("a").unwrap().page = 4;
    first.global_marks.remove(&'A');
    let mut second = baseline.clone();
    second.documents.get_mut("b").unwrap().page = 9;
    second.global_marks.insert('B', Location::default());
    let latest = merge_library(&baseline, &first, baseline.clone());
    let merged = merge_library(&baseline, &second, latest);
    assert_eq!(merged.documents["a"].page, 4);
    assert_eq!(merged.documents["b"].page, 9);
    assert!(!merged.global_marks.contains_key(&'A'));
    assert!(merged.global_marks.contains_key(&'B'));
    let unchanged = merge_library(&baseline, &baseline, merged);
    assert_eq!(unchanged.documents["a"].page, 4);
}

#[test]
fn native_outline_preserves_group_headings_without_destinations() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("outline.pdf");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> >>",
        "<< /Type /Outlines /First 5 0 R /Last 5 0 R /Count 3 >>",
        "<< /Title <FEFF00500061007200740020006F006E0065000000000000> /Parent 4 0 R /First 6 0 R /Last 6 0 R /Count 2 >>",
        "<< /Title (Chapter one) /Parent 5 0 R /Dest [3 0 R /Fit] /First 7 0 R /Last 7 0 R /Count 1 >>",
        "<< /Title (Section one) /Parent 6 0 R /Dest [3 0 R /XYZ 0 120 null] >>",
    ];
    let mut pdf = String::from("%PDF-1.7\n");
    let mut offsets = vec![];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
    }
    let xref = pdf.len();
    pdf.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    ));
    std::fs::write(&path, pdf).unwrap();
    let document = Document::open(&path).unwrap();
    assert_eq!(
        document
            .outline
            .iter()
            .map(|e| (e.title.as_str(), e.depth, e.page))
            .collect::<Vec<_>>(),
        vec![
            ("Part one", 0, 0),
            ("Chapter one", 1, 0),
            ("Section one", 2, 0)
        ]
    );
}

#[test]
fn bookmark_padding_cleanup_preserves_unicode_and_internal_replacement_characters() {
    use omapdf::document::clean_outline_title;
    assert_eq!(clean_outline_title("Preface\u{fffd}\u{fffd}\0 "), "Preface");
    assert_eq!(clean_outline_title("Résumé — 中文  "), "Résumé — 中文");
    assert_eq!(
        clean_outline_title("Before \u{fffd} after"),
        "Before \u{fffd} after"
    );
}

#[test]
fn original_pdf_fixture_bookmarks_are_clean_and_keep_their_levels() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/reading-lab.pdf");
    let document = Document::open_preview(&path, Default::default()).unwrap();
    assert!(
        document
            .outline
            .iter()
            .all(|entry| !entry.title.ends_with('\u{fffd}'))
    );
    let chapter = document
        .outline
        .iter()
        .find(|e| e.title == "1. READING LAB")
        .unwrap();
    let section = document
        .outline
        .iter()
        .find(|e| e.title == "1.1 History of a reading session")
        .unwrap();
    let subheading = document
        .outline
        .iter()
        .find(|e| e.title == "Starting points")
        .unwrap();
    assert_eq!(section.depth, chapter.depth + 1);
    assert_eq!(subheading.depth, section.depth + 1);
    assert_eq!(section.page, 29);
    assert!(document.outline.iter().any(|e| e.title == "Preface"));
}

#[test]
fn default_raster_matches_native_mupdf_pixels_without_post_processing() {
    use omapdf::{config::Config, raster::Rasterizer};
    let config = Config::default();
    assert_eq!(config.render_scale, 1.);
    assert_eq!(config.render_sharpness, 0.);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.pdf");
    write_pdf(&path);
    let document = std::sync::Arc::new(Document::open_preview(&path, Default::default()).unwrap());
    let rendered = Rasterizer::new().render(document, 0, 930, false).unwrap();
    let native = mupdf::Document::open(path.to_str().unwrap()).unwrap();
    let list = native.load_page(0).unwrap().to_display_list(true).unwrap();
    let bounds = list.bounds();
    let scale = 930. / (bounds.x1 - bounds.x0);
    let matrix = mupdf::Matrix::new(scale, 0., 0., scale, -bounds.x0 * scale, -bounds.y0 * scale);
    let pixels = list
        .to_pixmap(&matrix, &mupdf::Colorspace::device_bgr(), false)
        .unwrap();
    assert_eq!(rendered.size(0).width.0, pixels.width() as i32);
    assert_eq!(rendered.size(0).height.0, pixels.height() as i32);
    let mut expected = vec![];
    for row in pixels
        .samples()
        .chunks(pixels.stride() as usize)
        .take(pixels.height() as usize)
    {
        for pixel in row[..pixels.width() as usize * 3].as_chunks::<3>().0 {
            expected.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
    }
    assert_eq!(rendered.as_bytes(0).unwrap(), expected.as_slice());
}

#[test]
fn rounded_corners_and_document_theme_preferences_roundtrip() {
    use omapdf::config::Config;
    let directory = tempfile::tempdir().unwrap();
    let mut config = Config::load(directory.path()).unwrap();
    assert!(config.rounded_corners);
    assert!(!config.document_theme);
    assert_eq!(config.wheel_multiplier, 2.);
    config.set_preference("rounded_corners", "0").unwrap();
    config.set_preference("document_theme", "1").unwrap();
    let saved = Config::load(directory.path()).unwrap();
    assert!(!saved.rounded_corners && saved.document_theme);
    assert!(
        config
            .apply_preference("rounded_corners", "sometimes")
            .is_err()
    );
    assert!(config.apply_preference("document_theme", "dark").is_err());
}

#[test]
fn document_theme_cache_is_separate_and_reversible_for_pdf_and_epub() {
    use omapdf::raster::Rasterizer;
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("native-text.pdf");
    write_pdf(&pdf);
    for path in [
        pdf,
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/reading-lab.epub"),
    ] {
        let document =
            std::sync::Arc::new(Document::open_preview(&path, Default::default()).unwrap());
        let raster = Rasterizer::new();
        let original = raster.render(document.clone(), 0, 400, false).unwrap();
        raster.set_theme(Some((0x040816, 0xffcead)));
        let themed = raster.render(document.clone(), 0, 400, false).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&original, &themed));
        assert!(original.as_bytes(0) != themed.as_bytes(0));
        raster.set_theme(Some((0xfafafa, 0x202020)));
        let light = raster.render(document.clone(), 0, 400, false).unwrap();
        assert!(light.as_bytes(0) != themed.as_bytes(0));
        raster.set_theme(None);
        assert!(std::sync::Arc::ptr_eq(
            &original,
            &raster.render(document, 0, 400, false).unwrap()
        ));
    }
}

#[test]
fn themed_render_preserves_images_and_antialiases_colored_text_natively() {
    use omapdf::{document_theme::DocumentTheme, raster::Rasterizer};
    let dir = tempfile::tempdir().unwrap();
    let original_path = dir.path().join("original.pdf");
    let reference_path = dir.path().join("reference.pdf");
    let image = "q 120 0 0 120 40 20 cm BI /W 4 /H 2 /CS /RGB /BPC 8 /F /ASCIIHexDecode ID\n000000404040808080FFFFFF FF000000FF000000FFB0B0B0>\nEI Q\n";
    write_pdf_stream(
        &original_path,
        false,
        &format!("{image}0 0 1 rg BT /F1 24 Tf 10 170 Td (Blue heading) Tj ET"),
    );
    write_pdf_stream(
        &reference_path,
        false,
        &format!(
            "0.0627451 0.1254902 0.1882353 rg 0 0 300 200 re f\n{image}0.5019608 0.3764706 0.7529412 rg BT /F1 24 Tf 10 170 Td (Blue heading) Tj ET"
        ),
    );
    let doc =
        std::sync::Arc::new(Document::open_preview(&original_path, Default::default()).unwrap());
    let reference =
        std::sync::Arc::new(Document::open_preview(&reference_path, Default::default()).unwrap());
    let raster = Rasterizer::new();
    let original = raster.render(doc.clone(), 0, 600, false).unwrap();
    let expected = raster.render(reference, 0, 600, false).unwrap();
    raster.set_palette(Some(DocumentTheme::new(0x102030, 0xe0c0a0, 0x8060c0)));
    let actual = raster.render(doc.clone(), 0, 600, false).unwrap();
    let actual_pixels = actual.as_bytes(0).unwrap();
    let original_pixels = original.as_bytes(0).unwrap();
    for y in 125..355 {
        let row = (y * 600 + 85) * 4..(y * 600 + 315) * 4;
        assert_eq!(
            &actual_pixels[row.clone()],
            &original_pixels[row],
            "image pixels changed"
        );
    }
    for (actual, expected) in actual_pixels.iter().zip(expected.as_bytes(0).unwrap()) {
        assert!(
            actual.abs_diff(*expected) <= 1,
            "themed rendering differs from native reference: {actual} vs {expected}"
        );
    }
    raster.set_palette(Some(DocumentTheme::new(0x102030, 0xe0c0a0, 0xc08060)));
    let changed = raster.render(doc, 0, 600, false).unwrap();
    assert_ne!(
        actual_pixels,
        changed.as_bytes(0).unwrap(),
        "accent must invalidate cached colored text"
    );
}

#[test]
fn visual_ruler_crosses_pages_skips_empty_pages_and_clamps() {
    use omapdf::visual::{lines, navigate};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lines.pdf");
    write_pdf_stream(
        &path,
        true,
        "BT /F1 12 Tf 30 160 Td (First line) Tj 0 -20 Td (Second line) Tj 0 -20 Td (Third line) Tj ET\n",
    );
    let mut doc = Document::open_preview(&path, Default::default()).unwrap();
    let first = navigate(&doc, None, (0, [30., 40.]), 0).unwrap().unwrap();
    assert_eq!(first.page, 0);
    assert_eq!(first.word, 0);
    assert!(!doc.text_ready(1));
    assert!(
        omapdf::visual::navigate_until(
            &doc,
            Some(first),
            (0, [0., 0.]),
            100_000,
            &std::sync::atomic::AtomicBool::new(true)
        )
        .unwrap()
        .is_none()
    );
    assert!(
        !doc.text_ready(1),
        "cancelled ruler navigation must not load more pages"
    );
    let next_page = navigate(&doc, Some(first), (0, [0., 0.]), 3)
        .unwrap()
        .unwrap();
    assert_eq!(next_page.page, 1);
    assert_eq!(next_page.word, 0);
    assert!(doc.text_ready(1));
    assert_eq!(
        navigate(&doc, Some(next_page), (0, [0., 0.]), -3).unwrap(),
        Some(first)
    );
    let last = navigate(&doc, Some(first), (0, [0., 0.]), 100_000)
        .unwrap()
        .unwrap();
    assert_eq!(last.page, 1);
    assert_eq!(last.word, 4);
    assert_eq!(
        navigate(&doc, Some(last), (0, [0., 0.]), -100_000).unwrap(),
        Some(first)
    );
    doc.pages[0].words = Vec::new().into();
    assert_eq!(
        navigate(&doc, None, (0, [0., 0.]), 0).unwrap(),
        Some(lines(1, &doc.pages[1].words)[0])
    );
    doc.pages[1].words = Vec::new().into();
    assert!(navigate(&doc, None, (0, [0., 0.]), 10).unwrap().is_none());
}

#[test]
fn concurrent_epub_typography_and_pdf_rasters_match_sequential_results() {
    use omapdf::{document::EpubOptions, raster::Rasterizer};
    use std::sync::{Arc, Barrier};
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("concurrent.pdf");
    write_pdf(&pdf);
    let epub = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/reading-lab.epub");
    let options = [
        EpubOptions {
            line_height: 1.1,
            ..Default::default()
        },
        EpubOptions {
            line_height: 1.9,
            ..Default::default()
        },
    ];
    let signature = |doc: &Document| {
        (
            doc.pages.len(),
            [0, doc.pages.len() / 2, doc.pages.len() - 1].map(|page| {
                doc.pages[page]
                    .words
                    .iter()
                    .map(|word| (word.text.clone(), word.rect))
                    .collect::<Vec<_>>()
            }),
        )
    };
    let expected =
        options.map(|option| signature(&Document::open_with_options(&epub, option).unwrap()));
    assert_ne!(expected[0].0, expected[1].0);
    let barrier = Arc::new(Barrier::new(4));
    std::thread::scope(|scope| {
        for worker in 0..4 {
            let barrier = barrier.clone();
            let options = &options;
            let expected = &expected;
            let epub = &epub;
            let pdf = &pdf;
            scope.spawn(move || {
                barrier.wait();
                for iteration in 0..3 {
                    let index = (worker + iteration) % 2;
                    let doc = Document::open_with_options(epub, options[index]).unwrap();
                    assert_eq!(signature(&doc), expected[index]);
                    let pdf = Arc::new(Document::open_preview(pdf, Default::default()).unwrap());
                    let raster = Rasterizer::new();
                    let image = raster.render(pdf, 0, 300, false).unwrap();
                    assert_eq!(image.size(0).width.0, 300);
                    raster.shutdown();
                }
            });
        }
    });
}

#[test]
fn native_annotation_export_import_is_lossless_idempotent_and_does_not_overwrite() {
    use omapdf::{
        annotations,
        config::Config,
        state::{Bookmark, Highlight, Library, Location},
    };
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.pdf");
    write_pdf(&source);
    let source_bytes = std::fs::read(&source).unwrap();
    let doc = Document::open(&source).unwrap();
    let mut library = Library::default();
    library.touch(&doc.id, &source);
    let colors = Config::default().highlight_colors;
    library
        .documents
        .get_mut(&doc.id)
        .unwrap()
        .highlights
        .push(Highlight {
            meta: Default::default(),
            page: 0,
            text: "research".into(),
            kind: 'c',
            note: Some("Research note — 中文".into()),
            rects: vec![doc.pages[0].words[1].rect],
        });
    library.bookmarks.push(Bookmark {
        meta: Default::default(),
        name: "Bookmark — résumé".into(),
        location: Location {
            document: doc.id.clone(),
            page: 0,
            zoom: 1.,
            fit: true,
            offset: [0., -0.2],
        },
    });
    let output = directory.path().join("annotated.pdf");
    assert_eq!(
        annotations::embed(&doc, &library, &colors, &output).unwrap(),
        (1, 1)
    );
    assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
    assert!(annotations::embed(&doc, &library, &colors, &source).is_err());
    assert!(annotations::embed(&doc, &library, &colors, &output).is_err());
    let annotated = Document::open(&output).unwrap();
    let imported = annotations::read(&annotated, &colors).unwrap();
    assert_eq!(imported.highlights.len(), 1);
    assert_eq!(imported.bookmarks.len(), 1);
    assert_eq!(imported.highlights[0].text, "research");
    assert_eq!(imported.highlights[0].kind, 'c');
    assert_eq!(
        imported.highlights[0].note.as_deref(),
        Some("Research note — 中文")
    );
    assert_eq!(imported.bookmarks[0].name, "Bookmark — résumé");
    let mut imported_library = Library::default();
    imported_library.touch(&annotated.id, &output);
    assert_eq!(
        annotations::merge(&mut imported_library, &annotated.id, imported).unwrap(),
        (1, 1, 0)
    );
    assert_eq!(
        annotations::merge(
            &mut imported_library,
            &annotated.id,
            annotations::read(&annotated, &colors).unwrap()
        )
        .unwrap(),
        (0, 0, 0)
    );
    let second = directory.path().join("second-export.pdf");
    assert_eq!(
        annotations::embed(&annotated, &imported_library, &colors, &second).unwrap(),
        (0, 0)
    );
    assert_eq!(
        annotations::read(&Document::open(&second).unwrap(), &colors)
            .unwrap()
            .highlights
            .len(),
        1
    );
    imported_library
        .documents
        .get_mut(&annotated.id)
        .unwrap()
        .highlights[0]
        .note = Some("Edited embedded comment".into());
    let edited = directory.path().join("edited-comment.pdf");
    assert_eq!(
        annotations::embed(&annotated, &imported_library, &colors, &edited).unwrap(),
        (1, 0)
    );
    let edited_doc = Document::open(&edited).unwrap();
    let edited_annotations = annotations::read(&edited_doc, &colors).unwrap();
    assert_eq!(
        edited_annotations.highlights.len(),
        1,
        "editing must update rather than duplicate an embedded highlight"
    );
    assert_eq!(
        edited_annotations.highlights[0].note.as_deref(),
        Some("Edited embedded comment")
    );
    imported_library
        .documents
        .get_mut(&annotated.id)
        .unwrap()
        .highlights[0]
        .note = None;
    let cleared = directory.path().join("cleared-comment.pdf");
    assert_eq!(
        annotations::embed(&annotated, &imported_library, &colors, &cleared).unwrap(),
        (1, 0)
    );
    assert!(
        annotations::read(&Document::open(&cleared).unwrap(), &colors)
            .unwrap()
            .highlights[0]
            .note
            .is_none()
    );
    // Invalid saved annotation geometry cannot leave a partial output behind.
    library.documents.get_mut(&doc.id).unwrap().highlights[0].page = 999;
    let invalid = directory.path().join("invalid.pdf");
    assert!(annotations::embed(&doc, &library, &colors, &invalid).is_err());
    assert!(!invalid.exists());
    // Source changes invalidate the operation rather than embedding stale coordinates.
    std::fs::write(&source, b"partial rewrite").unwrap();
    assert!(
        annotations::embed(&doc, &library, &colors, &directory.path().join("stale.pdf")).is_err()
    );
}

#[test]
fn native_annotations_preserve_geometry_on_cropped_and_rotated_pages() {
    use omapdf::{
        annotations,
        config::Config,
        state::{Highlight, Library},
    };
    let directory = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let source = directory.path().join(format!("source-{rotation}.pdf"));
        write_pdf_stream_geometry(
            &source,
            false,
            "BT /F1 18 Tf 30 150 Td (Hello research world) Tj ET\n",
            &format!("/MediaBox [10 20 310 220] /CropBox [15 25 300 210] /Rotate {rotation}"),
        );
        let doc = Document::open(&source).unwrap();
        let rect = doc.pages[0].words[1].rect;
        let mut library = Library::default();
        library.touch(&doc.id, &source);
        library
            .documents
            .get_mut(&doc.id)
            .unwrap()
            .highlights
            .push(Highlight {
                meta: Default::default(),
                page: 0,
                text: "research".into(),
                kind: 'z',
                note: None,
                rects: vec![rect],
            });
        let colors = Config::default().highlight_colors;
        let output = directory.path().join(format!("export-{rotation}.pdf"));
        annotations::embed(&doc, &library, &colors, &output).unwrap();
        let exported = Document::open(&output).unwrap();
        let imported = annotations::read(&exported, &colors).unwrap();
        assert_eq!(imported.highlights[0].text, "research");
        assert_eq!(imported.highlights[0].kind, 'z');
        let actual = imported.highlights[0].rects[0];
        for delta in [
            rect.x - actual.x,
            rect.y - actual.y,
            rect.width - actual.width,
            rect.height - actual.height,
        ] {
            assert!(
                delta.abs() < 0.05,
                "rotation {rotation}: {rect:?} != {actual:?}"
            );
        }
    }
}

#[test]
fn native_json_backups_roundtrip_unavailable_documents_and_protect_source_files() {
    use omapdf::{
        interchange,
        state::{AnnotationMeta, Bookmark},
    };
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("research.pdf");
    write_pdf(&pdf);
    let doc = Document::open(&pdf).unwrap();
    let mut library = Library::default();
    library.touch(&doc.id, &pdf);
    library.documents.get_mut(&doc.id).unwrap().page = 0;
    library.documents.get_mut(&doc.id).unwrap().offset = [-0.1, -0.15];
    library.bookmarks.push(Bookmark {
        meta: AnnotationMeta::new(),
        name: "Résumé — 研究".into(),
        location: Location {
            document: doc.id.clone(),
            page: 0,
            zoom: 1.4,
            fit: false,
            offset: [0., -0.1],
        },
    });
    library.touch("missing-document", &directory.path().join("not-here.pdf"));
    library
        .source_imports
        .push(serde_json::json!({"documents":[],"extra_original_metadata":"retained"}));
    library.normalize_ids();
    let active = directory.path().join("library.json");
    library.save(&active).unwrap();
    let export = directory.path().join("backup.json");
    let issues = interchange::write(&export, &library, Default::default(), &active).unwrap();
    assert_eq!(issues.len(), 1);
    let report = interchange::read(&export, &Library::default(), Default::default()).unwrap();
    assert_eq!(report.library, library);
    let mut imported = Library::default();
    let counts = interchange::merge(&mut imported, report.library).unwrap();
    assert_eq!(counts.documents, 2);
    assert_eq!(counts.bookmarks, 1);
    assert_eq!(
        interchange::merge(&mut imported, library.clone()).unwrap(),
        Default::default()
    );
    // Repeated exports preserve the previous export as a separate recoverable file.
    let previous = std::fs::read(&export).unwrap();
    library.bookmarks[0].name = "Changed note".into();
    library.bookmarks[0].meta.touch();
    interchange::write(&export, &library, Default::default(), &active).unwrap();
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .flatten()
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("omapdf-export-backup-")
                && std::fs::read(entry.path()).unwrap() == previous)
    );
    let active_bytes = std::fs::read(&active).unwrap();
    assert!(interchange::write(&active, &library, Default::default(), &active).is_err());
    assert_eq!(std::fs::read(&active).unwrap(), active_bytes);
    let alias = directory.path().join("library-alias.json");
    std::fs::hard_link(&active, &alias).unwrap();
    assert!(interchange::write(&alias, &library, Default::default(), &active).is_err());
    let original = std::fs::read(&pdf).unwrap();
    assert!(interchange::write(&pdf, &library, Default::default(), &active).is_err());
    assert_eq!(std::fs::read(&pdf).unwrap(), original);
    let invalid = directory.path().join("future.json");
    std::fs::write(&invalid, r#"{"omapdf":{"version":999,"library":{}}}"#).unwrap();
    assert!(interchange::read(&invalid, &library, Default::default()).is_err());
    let invalid = directory.path().join("invalid.json");
    std::fs::write(
        &invalid,
        r#"{"documents":{"x":{"offset":[1e30,0] }},"bookmarks":[]}"#,
    )
    .unwrap();
    assert!(interchange::read(&invalid, &library, Default::default()).is_err());
}

#[test]
fn split_sqlite_migration_preserves_identity_coordinates_notes_portals_and_source_bytes() {
    use omapdf::{interchange, legacy_sqlite};
    use rusqlite::{Connection, params};
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("résumé's research.pdf");
    write_pdf_pages(&pdf, true);
    let other = directory.path().join("other.pdf");
    write_pdf_stream(
        &other,
        false,
        "BT /F1 18 Tf 30 150 Td (Other destination) Tj ET\n",
    );
    let doc = Document::open(&pdf).unwrap();
    let dest = Document::open(&other).unwrap();
    let checksum = interchange::checksum(&pdf).unwrap();
    let dest_checksum = interchange::checksum(&other).unwrap();
    let local = directory.path().join("local.db");
    let shared = directory.path().join("shared.db");
    {
        let connection = Connection::open(&local).unwrap();
        connection.execute_batch("CREATE TABLE document_hash(path TEXT,hash TEXT); CREATE TABLE \"extra metadata\"(payload BLOB);").unwrap();
        connection
            .execute(
                "INSERT INTO document_hash VALUES (?1,?2)",
                params![pdf.to_str().unwrap(), checksum],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO document_hash VALUES (?1,?2)",
                params![other.to_str().unwrap(), dest_checksum],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO \"extra metadata\" VALUES (?1)",
                params![&[0u8, 1, 255]],
            )
            .unwrap();
    }
    {
        let connection = Connection::open(&shared).unwrap();
        connection.execute_batch("CREATE TABLE opened_books(path TEXT,zoom_level REAL,offset_x REAL,offset_y REAL,last_access_time TEXT);
            CREATE TABLE bookmarks(document_path TEXT,desc TEXT,offset_y REAL,uuid TEXT,creation_time TEXT,modification_time TEXT);
            CREATE TABLE marks(document_path TEXT,symbol TEXT,offset_y REAL,offset_x REAL,zoom_level REAL,uuid TEXT);
            CREATE TABLE highlights(document_path TEXT,desc TEXT,text_annot TEXT,type TEXT,begin_x REAL,begin_y REAL,end_x REAL,end_y REAL,uuid TEXT,creation_time TEXT,modification_time TEXT);
            CREATE TABLE links(src_document TEXT,dst_document TEXT,src_offset_y REAL,src_offset_x REAL,dst_offset_x REAL,dst_offset_y REAL,dst_zoom_level REAL,uuid TEXT);").unwrap();
        connection
            .execute(
                "INSERT INTO opened_books VALUES (?1,1.7,12,260,'2026-10-01 12:00:00')",
                params![checksum],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO opened_books VALUES (?1,1,0,40,'2026-10-01 11:00:00')",
                params![dest_checksum],
            )
            .unwrap();
        connection.execute("INSERT INTO bookmarks VALUES (?1,'Unicode 研究 bookmark',240,'bookmark-uuid',NULL,NULL)",params![checksum]).unwrap();
        for (symbol, y) in [("a", 245.), ("G", 255.)] {
            connection
                .execute(
                    "INSERT INTO marks VALUES (?1,?2,?3,3,1.3,?4)",
                    params![checksum, symbol, y, format!("mark-{symbol}")],
                )
                .unwrap();
        }
        let rect = doc.pages[0].words[1].rect;
        connection.execute("INSERT INTO highlights VALUES (?1,'research','Note — résumé 研究','B',?2,?3,?4,?3,'highlight-uuid','2026-10-01 01:02:03','2026-10-02 04:05:06')",
            params![checksum,rect.x-150.+0.5,rect.y+rect.height/2.,rect.x+rect.width-150.-0.5]).unwrap();
        connection
            .execute(
                "INSERT INTO links VALUES (?1,?2,255,2,0,45,1.8,'portal-uuid')",
                params![checksum, dest_checksum],
            )
            .unwrap();
    }
    let before_local = std::fs::read(&local).unwrap();
    let before_shared = std::fs::read(&shared).unwrap();
    let raw = legacy_sqlite::read(directory.path()).unwrap();
    assert!(
        raw["sqlite_sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["tables"]["extra metadata"][0]["payload"]["blob_hex"] == "0001ff")
    );
    let report =
        interchange::read(directory.path(), &Library::default(), Default::default()).unwrap();
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let library = &report.library;
    assert_eq!(library.documents.len(), 2);
    let state = &library.documents[&doc.id];
    assert_eq!(state.page, 1);
    assert!((state.offset[1] + 0.2).abs() < 0.0001);
    assert_eq!(state.zoom, 1.7);
    assert_eq!(library.bookmarks[0].meta.uuid, "bookmark-uuid");
    assert_eq!(library.bookmarks[0].location.page, 1);
    assert_eq!(state.marks[&'a'].page, 1);
    assert_eq!(state.mark_metadata[&'a'].uuid, "mark-a");
    assert_eq!(library.global_marks[&'G'].page, 1);
    assert_eq!(state.highlights[0].kind, 'B');
    assert_eq!(state.highlights[0].text, "research");
    assert_eq!(state.highlights[0].meta.uuid, "highlight-uuid");
    assert_eq!(
        state.highlights[0].meta.creation_time,
        "2026-10-01 01:02:03"
    );
    assert_eq!(
        state.highlights[0].meta.modification_time,
        "2026-10-02 04:05:06"
    );
    assert_eq!(
        state.highlights[0].note.as_deref(),
        Some("Note — résumé 研究")
    );
    assert_eq!(state.portals[0].source_page, 1);
    assert!(state.portals[0].source_offset.is_some());
    assert_eq!(state.portals[0].destination.document, dest.id);
    assert_eq!(state.portals[0].meta.uuid, "portal-uuid");
    assert_eq!(std::fs::read(&local).unwrap(), before_local);
    assert_eq!(std::fs::read(&shared).unwrap(), before_shared);
    let mut merged = Library::default();
    interchange::merge(&mut merged, report.library).unwrap();
    let again = interchange::read_paths(&[shared, local], &merged, Default::default()).unwrap();
    assert_eq!(
        interchange::merge(&mut merged, again.library).unwrap(),
        Default::default()
    );
    assert_eq!(merged.documents[&doc.id].highlights.len(), 1);
}

#[test]
fn three_way_annotation_merge_retains_concurrent_additions_edits_and_deletions() {
    use omapdf::state::{AnnotationMeta, Highlight, merge_library};
    let mut baseline = Library::default();
    baseline.touch("doc", std::path::Path::new("doc.pdf"));
    let highlight = Highlight {
        meta: AnnotationMeta::new(),
        page: 0,
        text: "research".into(),
        kind: 'a',
        note: None,
        rects: vec![omapdf::document::Rect {
            x: 1.,
            y: 2.,
            width: 10.,
            height: 10.,
        }],
    };
    baseline
        .documents
        .get_mut("doc")
        .unwrap()
        .highlights
        .push(highlight.clone());
    let mut first = baseline.clone();
    let mut second = baseline.clone();
    let mut added = highlight.clone();
    added.meta = AnnotationMeta::new();
    added.page = 1;
    first
        .documents
        .get_mut("doc")
        .unwrap()
        .highlights
        .push(added);
    second.documents.get_mut("doc").unwrap().highlights[0].note =
        Some("Edited concurrently".into());
    let latest = merge_library(&baseline, &first, baseline.clone());
    let merged = merge_library(&baseline, &second, latest);
    assert_eq!(merged.documents["doc"].highlights.len(), 2);
    assert_eq!(
        merged.documents["doc"].highlights[0].note.as_deref(),
        Some("Edited concurrently")
    );
    let mut deleted = baseline.clone();
    deleted.documents.get_mut("doc").unwrap().highlights.clear();
    let latest = merge_library(&baseline, &deleted, baseline.clone());
    let merged = merge_library(&baseline, &second, latest);
    assert!(
        merged.documents["doc"].highlights.is_empty(),
        "stale edits must not resurrect another reader's deletion"
    );
    // Two readers introducing the same document must retain both annotation sets.
    let empty = Library::default();
    let mut first = baseline.clone();
    let mut second = baseline.clone();
    first.documents.get_mut("doc").unwrap().highlights[0].meta = AnnotationMeta::new();
    second.documents.get_mut("doc").unwrap().highlights[0].meta = AnnotationMeta::new();
    let latest = merge_library(&empty, &first, empty.clone());
    let merged = merge_library(&empty, &second, latest);
    assert_eq!(merged.documents["doc"].highlights.len(), 2);
}

#[test]
fn retained_legacy_records_retry_missing_paths_and_reject_mismatched_files() {
    use omapdf::interchange;
    let directory = tempfile::tempdir().unwrap();
    let template = directory.path().join("template.pdf");
    write_pdf(&template);
    let hash = interchange::checksum(&template).unwrap();
    let missing = directory.path().join("missing.pdf");
    let raw = serde_json::json!({"documents":[{"path":"missing.pdf","checksum":hash,"bookmarks":[{
        "uuid":"retained-bookmark","description":"Retained note — 研究","y_offset":50
    }]}]});
    let report = interchange::import_legacy(
        &raw,
        directory.path(),
        &Library::default(),
        Default::default(),
    )
    .unwrap();
    assert!(report.library.documents.is_empty());
    assert_eq!(report.issues.len(), 1);
    let mut known = report.library;
    assert_eq!(known.source_imports.len(), 1);
    // A different document at the old path must not receive the annotations.
    write_pdf_stream(
        &missing,
        false,
        "BT /F1 18 Tf 30 150 Td (Different document) Tj ET\n",
    );
    let failed = interchange::retry(&known, Default::default()).unwrap();
    assert!(failed.library.documents.is_empty());
    assert_eq!(failed.issues.len(), 1);
    std::fs::copy(&template, &missing).unwrap();
    let recovered = interchange::retry(&known, Default::default()).unwrap();
    assert!(recovered.issues.is_empty(), "{:?}", recovered.issues);
    assert_eq!(
        recovered.library.bookmarks[0].meta.uuid,
        "retained-bookmark"
    );
    interchange::merge(&mut known, recovered.library).unwrap();
    assert_eq!(known.source_imports.len(), 1);
    let recovered_id = known.bookmarks[0].location.document.clone();
    assert_eq!(
        known.documents[&recovered_id].path,
        missing.canonicalize().unwrap()
    );
    // Relocation can also be resolved from an already-known document checksum.
    std::fs::remove_file(&missing).unwrap();
    let doc = Document::open(&template).unwrap();
    known.touch(&doc.id, &template);
    let relocated = interchange::retry(&known, Default::default()).unwrap();
    assert!(relocated.issues.is_empty(), "{:?}", relocated.issues);
    assert_eq!(
        relocated.library.bookmarks[0].meta.uuid,
        "retained-bookmark"
    );
}

#[test]
fn original_json_coordinates_and_metadata_survive_compatible_export_import() {
    use omapdf::interchange;
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("two.pdf");
    write_pdf_pages(&pdf, true);
    let doc = Document::open(&pdf).unwrap();
    let rect = doc.pages[1].words[1].rect;
    let raw = serde_json::json!({"documents":[{"path":pdf,"checksum":interchange::checksum(&pdf).unwrap(),
        "offset_y":260.,"offset_x":12.,"zoom_level":1.6,
        "bookmarks":[{"uuid":"bookmark-json","description":"Unicode résumé","y_offset":250.}],
        "marks":[{"uuid":"mark-json","symbol":97,"y_offset":240.,"x_offset":9.,"zoom_level":1.4}],
        "highlights":[{"uuid":"highlight-json","type":97,"description":"research","text_annot":"Comment — 研究",
            "creation_time":"2026-10-01 10:20:30","modification_time":"2026-10-02 12:34:56",
            "selection_begin_x":rect.x-150.+0.5,"selection_begin_y":200.+rect.y+rect.height/2.,
            "selection_end_x":rect.x+rect.width-150.-0.5,"selection_end_y":200.+rect.y+rect.height/2.}]}]});
    let imported = interchange::import_legacy(
        &raw,
        directory.path(),
        &Library::default(),
        Default::default(),
    )
    .unwrap();
    assert!(imported.issues.is_empty(), "{:?}", imported.issues);
    let (mut exported, issues) =
        interchange::export_value(&imported.library, Default::default()).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    exported.as_object_mut().unwrap().remove("omapdf");
    let again = interchange::import_legacy(
        &exported,
        directory.path(),
        &Library::default(),
        Default::default(),
    )
    .unwrap();
    assert!(again.issues.is_empty(), "{:?}", again.issues);
    let original = &imported.library.documents[&doc.id];
    let restored = &again.library.documents[&doc.id];
    assert_eq!(original.page, restored.page);
    assert_eq!(original.offset, restored.offset);
    assert_eq!(original.zoom, restored.zoom);
    assert_eq!(original.highlights, restored.highlights);
    assert_eq!(original.marks, restored.marks);
    assert_eq!(original.mark_metadata, restored.mark_metadata);
    assert_eq!(imported.library.bookmarks, again.library.bookmarks);
}

#[test]
fn old_combined_sqlite_layout_and_live_wal_are_read_without_modification() {
    use omapdf::interchange;
    use rusqlite::{Connection, params};
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("old.pdf");
    write_pdf(&pdf);
    let database = directory.path().join("test.db");
    let connection = Connection::open(&database).unwrap();
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .unwrap();
    connection
        .execute_batch(
            "CREATE TABLE opened_books(path TEXT,zoom_level REAL,offset_x REAL,offset_y REAL);
        CREATE TABLE bookmarks(document_path TEXT,desc TEXT,offset_y REAL);",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO opened_books VALUES (?1,1.5,0,60)",
            params![pdf.to_str().unwrap()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO bookmarks VALUES (?1,'WAL-only note',45)",
            params![pdf.to_str().unwrap()],
        )
        .unwrap();
    let before = std::fs::read(&database).unwrap();
    let wal = std::fs::read(database.with_extension("db-wal")).unwrap();
    let report = interchange::read(&database, &Library::default(), Default::default()).unwrap();
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    assert_eq!(report.library.bookmarks.len(), 1);
    assert_eq!(report.library.bookmarks[0].name, "WAL-only note");
    assert!(!report.library.bookmarks[0].meta.uuid.is_empty());
    assert_eq!(std::fs::read(&database).unwrap(), before);
    assert_eq!(
        std::fs::read(database.with_extension("db-wal")).unwrap(),
        wal
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM bookmarks", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn old_duplicate_annotations_gain_distinct_stable_ids_and_remain_individually_deletable() {
    use omapdf::state::merge_library;
    let value = serde_json::json!({"documents":{"doc":{"path":"doc.pdf","highlights":[
        {"page":0,"text":"same","kind":"a","rects":[{"x":1,"y":2,"width":3,"height":4}]},
        {"page":0,"text":"same","kind":"a","rects":[{"x":1,"y":2,"width":3,"height":4}]}
    ]}},"bookmarks":[]});
    let mut old: Library = serde_json::from_value(value.clone()).unwrap();
    old.normalize_ids();
    let mut another: Library = serde_json::from_value(value).unwrap();
    another.normalize_ids();
    assert_eq!(old, another);
    let highlights = &old.documents["doc"].highlights;
    assert_ne!(highlights[0].meta.uuid, highlights[1].meta.uuid);
    let mut edit = old.clone();
    edit.documents.get_mut("doc").unwrap().highlights.remove(0);
    let merged = merge_library(&old, &edit, old.clone());
    assert_eq!(merged.documents["doc"].highlights.len(), 1);
}

#[test]
fn newer_imported_annotation_revisions_update_without_clobbering_newer_local_comments() {
    use omapdf::{
        interchange,
        state::{AnnotationMeta, Highlight},
    };
    let mut base = Library::default();
    base.touch("doc", std::path::Path::new("doc.pdf"));
    base.documents
        .get_mut("doc")
        .unwrap()
        .highlights
        .push(Highlight {
            meta: AnnotationMeta::new(),
            page: 0,
            text: "research".into(),
            kind: 'a',
            note: Some("original".into()),
            rects: vec![omapdf::document::Rect {
                x: 1.,
                y: 2.,
                width: 3.,
                height: 4.,
            }],
        });
    let mut local = base.clone();
    let highlight = &mut local.documents.get_mut("doc").unwrap().highlights[0];
    highlight.note = Some("local edit".into());
    highlight.meta.touch();
    interchange::merge(&mut local, base.clone()).unwrap();
    assert_eq!(
        local.documents["doc"].highlights[0].note.as_deref(),
        Some("local edit")
    );
    let mut newer = base;
    let highlight = &mut newer.documents.get_mut("doc").unwrap().highlights[0];
    highlight.note = Some("newer imported edit".into());
    highlight.meta.touch();
    interchange::merge(&mut local, newer).unwrap();
    assert_eq!(local.documents["doc"].highlights.len(), 1);
    assert_eq!(
        local.documents["doc"].highlights[0].note.as_deref(),
        Some("newer imported edit")
    );
}

#[test]
fn native_markup_styles_and_annotation_identities_survive_pdf_roundtrip() {
    use mupdf::pdf::{PdfAnnotationType, PdfDocument, PdfPage};
    use omapdf::{
        annotations,
        config::Config,
        state::{AnnotationMeta, Highlight},
    };
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("styles.pdf");
    write_pdf(&source);
    let doc = Document::open(&source).unwrap();
    let mut library = Library::default();
    library.touch(&doc.id, &source);
    let kinds = ['a', 'B', '_'];
    for (index, kind) in kinds.into_iter().enumerate() {
        library
            .documents
            .get_mut(&doc.id)
            .unwrap()
            .highlights
            .push(Highlight {
                meta: AnnotationMeta::new(),
                page: 0,
                text: doc.pages[0].words[index].text.clone(),
                kind,
                note: Some(format!("Note {kind}")),
                rects: vec![doc.pages[0].words[index].rect],
            });
    }
    let output = directory.path().join("marked.pdf");
    let colors = Config::default().highlight_colors;
    assert_eq!(
        annotations::embed(&doc, &library, &colors, &output).unwrap(),
        (3, 0)
    );
    let marked = Document::open(&output).unwrap();
    let imported = annotations::read(&marked, &colors).unwrap();
    assert_eq!(imported.unsupported, 0);
    assert_eq!(imported.highlights.len(), 3);
    for (expected, actual) in library.documents[&doc.id]
        .highlights
        .iter()
        .zip(&imported.highlights)
    {
        assert_eq!(expected.kind, actual.kind);
        assert_eq!(expected.meta, actual.meta);
        assert_eq!(expected.note, actual.note);
    }
    let native = PdfDocument::open(output.to_str().unwrap()).unwrap();
    let page = PdfPage::try_from(native.load_page(0).unwrap()).unwrap();
    let types = page
        .annotations()
        .map(|annotation| annotation.r#type().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        types,
        [
            PdfAnnotationType::Highlight,
            PdfAnnotationType::Underline,
            PdfAnnotationType::StrikeOut
        ]
    );
}

#[test]
fn deletion_records_survive_old_imports_backups_and_stale_identity_aliases() {
    use omapdf::{
        interchange,
        state::{AnnotationMeta, Highlight, merge_library},
    };
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("doc.pdf");
    write_pdf(&pdf);
    let doc = Document::open(&pdf).unwrap();
    let mut library = Library::default();
    library.touch(&doc.id, &pdf);
    library
        .documents
        .get_mut(&doc.id)
        .unwrap()
        .highlights
        .push(Highlight {
            meta: AnnotationMeta::new(),
            page: 0,
            text: "research".into(),
            kind: 'a',
            note: None,
            rects: vec![doc.pages[0].words[1].rect],
        });
    let old = library.clone();
    let uuid = old.documents[&doc.id].highlights[0].meta.uuid.clone();
    assert!(library.delete_annotation(&uuid));
    assert!(library.documents[&doc.id].highlights.is_empty());
    interchange::merge(&mut library, old.clone()).unwrap();
    assert!(library.documents[&doc.id].highlights.is_empty());
    let (backup, _) = interchange::export_value(&library, Default::default()).unwrap();
    let path = directory.path().join("export.json");
    std::fs::write(&path, serde_json::to_vec(&backup).unwrap()).unwrap();
    let restored = interchange::read(&path, &Library::default(), Default::default()).unwrap();
    assert_eq!(
        restored.library.deleted_annotations,
        library.deleted_annotations
    );
    let mut from_backup = restored.library;
    interchange::merge(&mut from_backup, old.clone()).unwrap();
    assert!(from_backup.documents[&doc.id].highlights.is_empty());
    // A stale reader that did not know a newly-linked PDF identity must delete both names.
    let mut latest = old.clone();
    latest.documents.get_mut(&doc.id).unwrap().highlights[0]
        .meta
        .aliases
        .insert("pdf-identity".into());
    let merged = merge_library(&old, &library, latest);
    assert!(merged.deleted_annotations.contains_key("pdf-identity"));
    let mut incoming = old;
    incoming.documents.get_mut(&doc.id).unwrap().highlights[0]
        .meta
        .uuid = "pdf-identity".into();
    let mut merged = merged;
    interchange::merge(&mut merged, incoming).unwrap();
    assert!(merged.documents[&doc.id].highlights.is_empty());
}

#[test]
fn legacy_highlights_without_uuids_update_comments_without_duplicate_imports() {
    use omapdf::interchange;
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("doc.pdf");
    write_pdf(&pdf);
    let doc = Document::open(&pdf).unwrap();
    let rect = doc.pages[0].words[1].rect;
    let mut raw = serde_json::json!({"documents":[{"path":pdf,"checksum":interchange::checksum(&pdf).unwrap(),"highlights":[{
        "type":97,"description":"research","text_annot":"first comment","selection_begin_x":rect.x-150.+0.5,
        "selection_begin_y":rect.y+rect.height/2.,"selection_end_x":rect.x+rect.width-150.-0.5,"selection_end_y":rect.y+rect.height/2.
    }]}]});
    let mut library = interchange::import_legacy(
        &raw,
        directory.path(),
        &Library::default(),
        Default::default(),
    )
    .unwrap()
    .library;
    let uuid = library.documents[&doc.id].highlights[0].meta.uuid.clone();
    raw["documents"][0]["highlights"][0]["text_annot"] = serde_json::json!("updated comment");
    let next =
        interchange::import_legacy(&raw, directory.path(), &library, Default::default()).unwrap();
    let counts = interchange::merge(&mut library, next.library).unwrap();
    assert_eq!(counts.highlights, 0);
    assert_eq!(counts.updated, 1);
    assert_eq!(library.documents[&doc.id].highlights.len(), 1);
    assert_eq!(library.documents[&doc.id].highlights[0].meta.uuid, uuid);
    assert_eq!(
        library.documents[&doc.id].highlights[0].note.as_deref(),
        Some("updated comment")
    );
}

#[test]
fn external_pdf_names_are_page_scoped_and_deletion_only_changes_exported_copy() {
    use mupdf::pdf::{PdfAnnotationType, PdfDocument, PdfPage};
    use omapdf::{annotations, config::Config};
    let directory = tempfile::tempdir().unwrap();
    let fixture = directory.path().join("fixture.pdf");
    write_pdf_pages(&fixture, true);
    let external = directory.path().join("external.pdf");
    {
        let native = PdfDocument::open(fixture.to_str().unwrap()).unwrap();
        for index in 0..2 {
            let mut page = PdfPage::try_from(native.load_page(index).unwrap()).unwrap();
            let mut note = page.create_annotation(PdfAnnotationType::Text).unwrap();
            note.set_rect(mupdf::Rect::new(20., 30., 38., 48.)).unwrap();
            note.set_contents(&format!("Page {index} note")).unwrap();
            note.object()
                .dict_put("NM", native.new_string("page-local-name").unwrap())
                .unwrap();
            note.update().unwrap();
            page.update().unwrap();
        }
        native.save(external.to_str().unwrap()).unwrap();
    }
    let source_bytes = std::fs::read(&external).unwrap();
    let doc = Document::open(&external).unwrap();
    let colors = Config::default().highlight_colors;
    let imported = annotations::read(&doc, &colors).unwrap();
    assert_eq!(imported.bookmarks.len(), 2);
    assert_ne!(
        imported.bookmarks[0].meta.uuid,
        imported.bookmarks[1].meta.uuid
    );
    assert_eq!(
        imported.bookmarks[0].meta.pdf_name.as_deref(),
        Some("page-local-name")
    );
    let mut library = Library::default();
    library.touch(&doc.id, &external);
    annotations::merge(&mut library, &doc.id, imported).unwrap();
    let expected_id = library.bookmarks[1].meta.uuid.clone();
    let deleted = library.bookmarks[0].meta.uuid.clone();
    assert!(library.delete_annotation(&deleted));
    assert_eq!(
        annotations::merge(
            &mut library,
            &doc.id,
            annotations::read(&doc, &colors).unwrap()
        )
        .unwrap(),
        (0, 0, 0)
    );
    let output = directory.path().join("deleted.pdf");
    annotations::embed(&doc, &library, &colors, &output).unwrap();
    assert_eq!(std::fs::read(&external).unwrap(), source_bytes);
    let result = annotations::read(&Document::open(&output).unwrap(), &colors).unwrap();
    assert_eq!(result.bookmarks.len(), 1);
    assert_eq!(result.bookmarks[0].location.page, 1);
    assert_eq!(result.bookmarks[0].meta.uuid, expected_id);
    assert_eq!(
        result.bookmarks[0].meta.pdf_name.as_deref(),
        Some("page-local-name")
    );
}

#[test]
fn portal_proximity_distinguishes_sources_even_when_destinations_are_identical() {
    use omapdf::state::{AnnotationMeta, Portal, nearest_portal_index};
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("doc.pdf");
    write_pdf_pages(&pdf, true);
    let doc = Document::open(&pdf).unwrap();
    let target = Location {
        document: doc.id.clone(),
        page: 0,
        ..Default::default()
    };
    let portals = vec![
        Portal {
            meta: AnnotationMeta::new(),
            source_page: 1,
            source_offset: Some([0., -0.5]),
            destination: target.clone(),
        },
        Portal {
            meta: AnnotationMeta::new(),
            source_page: 1,
            source_offset: Some([0., -0.1]),
            destination: target,
        },
    ];
    let current = Location {
        document: doc.id.clone(),
        page: 1,
        offset: [0., -0.12],
        ..Default::default()
    };
    assert_eq!(nearest_portal_index(&doc, &current, &portals), Some(1));
}

#[test]
fn cross_page_highlights_keep_one_identity_through_edits_exports_and_deletion() {
    use omapdf::{annotations, config::Config, interchange};
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("spanning.pdf");
    write_pdf_pages(&pdf, true);
    let doc = Document::open(&pdf).unwrap();
    let first = doc.pages[0].words[1].rect;
    let last = doc.pages[1].words[1].rect;
    let raw = serde_json::json!({"documents":[{"path":pdf,"checksum":interchange::checksum(&pdf).unwrap(),"highlights":[{
        "uuid":"spanning-original","type":97,"text_annot":"Original note",
        "selection_begin_x":first.x-150.+0.5,"selection_begin_y":first.y+first.height/2.,
        "selection_end_x":last.x+last.width-150.-0.5,"selection_end_y":200.+last.y+last.height/2.
    }]}]});
    let imported = interchange::import_legacy(
        &raw,
        directory.path(),
        &Library::default(),
        Default::default(),
    )
    .unwrap();
    assert!(imported.issues.is_empty(), "{:?}", imported.issues);
    let mut library = imported.library;
    let fragments = &library.documents[&doc.id].highlights;
    assert_eq!(fragments.len(), 2);
    assert_ne!(fragments[0].meta.uuid, fragments[1].meta.uuid);
    assert!(
        fragments
            .iter()
            .all(|fragment| fragment.meta.group.as_deref() == Some("spanning-original"))
    );
    let selected = fragments[1].meta.clone();
    // Saved fragment order must not affect list ranges or next-highlight targets.
    library
        .documents
        .get_mut(&doc.id)
        .unwrap()
        .highlights
        .reverse();
    let groups = library.documents[&doc.id].highlight_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0]
            .iter()
            .map(|fragment| fragment.page)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    library
        .documents
        .get_mut(&doc.id)
        .unwrap()
        .highlights
        .reverse();
    assert!(library.set_highlight_note(&doc.id, &selected, "Edited — 研究"));
    assert!(
        library.documents[&doc.id]
            .highlights
            .iter()
            .all(|fragment| fragment.note.as_deref() == Some("Edited — 研究"))
    );
    let (mut exported, issues) = interchange::export_value(&library, Default::default()).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        exported["documents"][0]["highlights"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        exported["documents"][0]["highlights"][0]["uuid"],
        "spanning-original"
    );
    exported.as_object_mut().unwrap().remove("omapdf");
    let restored = interchange::import_legacy(
        &exported,
        directory.path(),
        &Library::default(),
        Default::default(),
    )
    .unwrap()
    .library;
    assert_eq!(
        library.documents[&doc.id].highlights,
        restored.documents[&doc.id].highlights
    );
    // PDF fragment aliases must not turn distinct pages into one identity on original-format round trips.
    let mut aliased = library.clone();
    for (index, highlight) in aliased
        .documents
        .get_mut(&doc.id)
        .unwrap()
        .highlights
        .iter_mut()
        .enumerate()
    {
        highlight
            .meta
            .aliases
            .insert(format!("external-page-{index}"));
    }
    let (mut original_json, _) = interchange::export_value(&aliased, Default::default()).unwrap();
    original_json.as_object_mut().unwrap().remove("omapdf");
    let incoming = interchange::import_legacy(
        &original_json,
        directory.path(),
        &aliased,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        interchange::merge(&mut aliased, incoming.library)
            .unwrap()
            .highlights,
        0
    );
    assert_eq!(aliased.documents[&doc.id].highlights.len(), 2);
    let output = directory.path().join("embedded.pdf");
    let colors = Config::default().highlight_colors;
    annotations::embed(&doc, &library, &colors, &output).unwrap();
    let embedded_doc = Document::open(&output).unwrap();
    let embedded = annotations::read(&embedded_doc, &colors).unwrap();
    assert_eq!(embedded.highlights.len(), 2);
    assert!(
        embedded
            .highlights
            .iter()
            .all(
                |fragment| fragment.meta.group.as_deref() == Some("spanning-original")
                    && fragment.note.as_deref() == Some("Edited — 研究")
            )
    );
    assert!(library.delete_annotation(&selected.uuid));
    assert!(library.documents[&doc.id].highlights.is_empty());
    assert!(
        library
            .deleted_annotations
            .contains_key("spanning-original")
    );
    let stale =
        interchange::import_legacy(&raw, directory.path(), &library, Default::default()).unwrap();
    interchange::merge(&mut library, stale.library).unwrap();
    assert!(library.documents[&doc.id].highlights.is_empty());
    // A source PDF with embedded fragments also respects the logical deletion.
    library.touch(&embedded_doc.id, &output);
    annotations::merge(&mut library, &embedded_doc.id, embedded).unwrap();
    assert!(library.documents[&embedded_doc.id].highlights.is_empty());
}

#[test]
fn editing_overlapping_highlights_uses_identity_and_does_not_resurrect_deleted_notes() {
    use omapdf::state::{AnnotationMeta, Highlight};
    let mut library = Library::default();
    let first = AnnotationMeta::new();
    let second = AnnotationMeta::new();
    let state = library.documents.entry("book".into()).or_default();
    for meta in [&first, &second] {
        state.highlights.push(Highlight {
            meta: meta.clone(),
            page: 0,
            text: "same text".into(),
            kind: 'a',
            note: None,
            rects: vec![],
        });
    }
    assert!(library.set_highlight_note("book", &second, "second comment"));
    assert_eq!(library.documents["book"].highlights[0].note, None);
    assert_eq!(
        library.documents["book"].highlights[1].note.as_deref(),
        Some("second comment")
    );
    assert!(library.delete_annotation(&second.uuid));
    assert!(!library.set_highlight_note("book", &second, "stale edit"));
    assert_eq!(library.documents["book"].highlights.len(), 1);
    assert_eq!(library.documents["book"].highlights[0].note, None);
}

#[test]
fn encrypted_documents_authenticate_every_native_worker_without_persisting_credentials() {
    use mupdf::pdf::{Encryption, PdfDocument, PdfWriteOptions};
    use omapdf::{annotations, config::Config, document::PasswordRequired, state::AnnotationMeta};
    let directory = tempfile::tempdir().unwrap();
    let plain = directory.path().join("plain.pdf");
    write_pdf_pages(&plain, true);
    let encrypted = directory.path().join("locked.pdf");
    {
        let native = PdfDocument::open(plain.to_str().unwrap()).unwrap();
        let mut options = PdfWriteOptions::default();
        options
            .set_encryption(Encryption::Aes256)
            .set_user_password("private-reader-secret")
            .set_owner_password("private-owner-secret");
        native
            .save_with_options(encrypted.to_str().unwrap(), options)
            .unwrap();
    }
    let bytes = std::fs::read(&encrypted).unwrap();
    let missing = Document::open(&encrypted).unwrap_err();
    assert!(!missing.downcast_ref::<PasswordRequired>().unwrap().rejected);
    let wrong = Document::unlock_preview(&encrypted, Default::default(), "wrong-secret".into())
        .unwrap_err();
    assert!(wrong.downcast_ref::<PasswordRequired>().unwrap().rejected);
    assert!(!format!("{wrong:?}").contains("wrong-secret"));
    let doc = Document::unlock_preview(
        &encrypted,
        Default::default(),
        "private-reader-secret".into(),
    )
    .unwrap();
    assert!(!format!("{doc:?}").contains("private-reader-secret"));
    assert_eq!(doc.pages.len(), 2);
    doc.ensure_page_text(1).unwrap();
    assert!(doc.pages[1].text().contains("research"));
    assert!(doc.render(1, 400, directory.path(), false).is_ok());
    let reopened = Document::open(&encrypted).unwrap();
    assert_eq!(doc.id, reopened.id);
    let colors = Config::default().highlight_colors;
    assert!(annotations::read(&doc, &colors).is_ok());
    let mut library = Library::default();
    library.touch(&doc.id, &encrypted);
    library.bookmarks.push(omapdf::state::Bookmark {
        meta: AnnotationMeta::new(),
        name: "Saved encrypted note".into(),
        location: Location {
            document: doc.id.clone(),
            page: 1,
            ..Default::default()
        },
    });
    let state = directory.path().join("library.json");
    library.save(&state).unwrap();
    assert!(
        !std::fs::read_to_string(&state)
            .unwrap()
            .contains("private-reader-secret")
    );
    let exported = directory.path().join("exported.pdf");
    annotations::embed(&doc, &library, &colors, &exported).unwrap();
    let mut native = PdfDocument::open(exported.to_str().unwrap()).unwrap();
    assert!(
        native.needs_password().unwrap(),
        "Export must preserve PDF encryption"
    );
    assert!(native.authenticate("private-reader-secret").unwrap());
    assert_eq!(std::fs::read(&encrypted).unwrap(), bytes);
    drop(reopened);
    drop(doc);
    assert!(
        Document::open(&encrypted)
            .unwrap_err()
            .downcast_ref::<PasswordRequired>()
            .is_some(),
        "Credentials must expire with the last document owner"
    );
    let owner = Document::unlock_preview(
        &encrypted,
        Default::default(),
        "private-owner-secret".into(),
    )
    .unwrap();
    assert!(owner.render(0, 300, directory.path(), false).is_ok());
}

#[test]
fn native_password_unlocking_covers_pdf_encryption_variants_and_empty_user_passwords() {
    use mupdf::pdf::{Encryption, PdfDocument, PdfWriteOptions};
    let directory = tempfile::tempdir().unwrap();
    let plain = directory.path().join("plain.pdf");
    write_pdf(&plain);
    for (index, encryption, password) in [
        (0, Encryption::Rc4_40, "reader"),
        (1, Encryption::Rc4_128, "reader"),
        (2, Encryption::Aes128, "reader"),
        (3, Encryption::Aes256, "résumé-研究"),
        (4, Encryption::Aes256, ""),
    ] {
        let output = directory.path().join(format!("locked-{index}.pdf"));
        {
            let native = PdfDocument::open(plain.to_str().unwrap()).unwrap();
            let mut options = PdfWriteOptions::default();
            options
                .set_encryption(encryption)
                .set_user_password(password)
                .set_owner_password("owner");
            native
                .save_with_options(output.to_str().unwrap(), options)
                .unwrap();
        }
        let doc = if password.is_empty() {
            Document::open(&output).unwrap()
        } else {
            assert!(Document::open(&output).is_err());
            Document::unlock_preview(&output, Default::default(), password.into()).unwrap()
        };
        doc.ensure_page_text(0).unwrap();
        assert!(doc.pages[0].text().contains("research"));
        assert!(doc.render(0, 300, directory.path(), false).is_ok());
    }
}

#[test]
fn image_regions_preserve_cross_page_geometry_gaps_density_and_markup() {
    use image::{Rgba, RgbaImage};
    use omapdf::{
        document::Rect,
        region::{Overlay, Placement, Region},
    };
    let pages = [
        Placement {
            page: 0,
            bounds: Rect {
                x: 10.,
                y: 10.,
                width: 150.,
                height: 200.,
            },
            size: [300., 400.],
        },
        Placement {
            page: 1,
            bounds: Rect {
                x: 190.,
                y: 10.,
                width: 150.,
                height: 200.,
            },
            size: [300., 400.],
        },
    ];
    let viewport = Rect {
        x: 0.,
        y: 0.,
        width: 400.,
        height: 240.,
    };
    let mut selected =
        Region::select([330., 180.], [20., 20.], viewport, 2., 0x112233, &pages).unwrap();
    assert_eq!((selected.width, selected.height), (620, 320));
    assert_eq!(selected.slices.len(), 2);
    assert_eq!(
        selected.slices[0].source,
        Rect {
            x: 20.,
            y: 20.,
            width: 280.,
            height: 320.
        }
    );
    assert_eq!(selected.slices[1].target.x, 340.);
    let forward = Region::select([20., 20.], [330., 180.], viewport, 2., 0x112233, &pages).unwrap();
    assert_eq!(forward.slices[0].source, selected.slices[0].source);
    selected.slices[0].overlays.push(Overlay {
        rect: Rect {
            x: 20.,
            y: 20.,
            width: 100.,
            height: 100.,
        },
        color: 0x00ff0080,
    });
    let image = selected
        .compose(|slice| {
            let mut image = RgbaImage::from_pixel(
                300,
                400,
                if slice.page == 0 {
                    Rgba([255, 0, 0, 255])
                } else {
                    Rgba([0, 0, 255, 255])
                },
            );
            for y in 0..20 {
                for x in 0..300 {
                    image.put_pixel(x, y, Rgba([0, 255, 0, 255]));
                }
            }
            Ok(image)
        })
        .unwrap();
    assert_eq!(*image.get_pixel(10, 10), Rgba([127, 128, 0, 255]));
    assert_eq!(*image.get_pixel(279, 100), Rgba([255, 0, 0, 255]));
    assert_eq!(*image.get_pixel(280, 100), Rgba([17, 34, 51, 255]));
    assert_eq!(*image.get_pixel(339, 100), Rgba([17, 34, 51, 255]));
    assert_eq!(*image.get_pixel(340, 100), Rgba([0, 0, 255, 255]));
    assert_eq!(*image.get_pixel(619, 319), Rgba([0, 0, 255, 255]));
    assert!(Region::select([0., 0.], [0., 0.], viewport, 1., 0, &pages).is_err());
    assert!(Region::select([f32::NAN, 0.], [10., 10.], viewport, 1., 0, &pages).is_err());
    assert!(Region::select([170., 20.], [180., 50.], viewport, 1., 0, &pages).is_err());
    assert!(
        Region::select(
            [0., 0.],
            [10000., 10000.],
            Rect {
                x: 0.,
                y: 0.,
                width: 10000.,
                height: 10000.
            },
            2.,
            0,
            &pages
        )
        .is_err()
    );
    assert!(Region::select([0., 0.], [20., 20.], viewport, f32::INFINITY, 0, &pages).is_err());
}

#[test]
fn native_image_regions_match_rotated_cropped_pdf_pixels_and_never_overwrite_files() {
    use omapdf::{
        document::Rect,
        raster::Rasterizer,
        region::{Placement, Region},
    };
    use std::sync::Arc;
    let directory = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let pdf = directory.path().join(format!("crop-{rotation}.pdf"));
        write_pdf_stream_geometry(
            &pdf,
            false,
            "1 0 0 rg 40 30 100 80 re f\n0 0 1 rg 160 70 150 120 re f\n",
            &format!("/MediaBox [0 0 400 300] /CropBox [40 30 340 230] /Rotate {rotation}"),
        );
        let bytes = std::fs::read(&pdf).unwrap();
        let doc = Arc::new(Document::open(&pdf).unwrap());
        let page = &doc.pages[0];
        let placements = [Placement {
            page: 0,
            bounds: Rect {
                x: 50.,
                y: 100.,
                width: page.width,
                height: page.height,
            },
            size: [page.width, page.height],
        }];
        let region = Region::select(
            [70., 130.],
            [170., 210.],
            Rect {
                x: 0.,
                y: 0.,
                width: 1000.,
                height: 1000.,
            },
            1.,
            0,
            &placements,
        )
        .unwrap();
        let renderer = Rasterizer::new();
        let full = renderer
            .render(doc.clone(), 0, page.width as u32, false)
            .unwrap();
        let mut rgba = full.as_bytes(0).unwrap().to_vec();
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let size = full.size(0);
        let full =
            image::RgbaImage::from_raw(size.width.0 as u32, size.height.0 as u32, rgba).unwrap();
        let encoded = region.render(doc.clone(), &renderer, false).unwrap();
        let actual = image::load_from_memory(&encoded).unwrap().to_rgba8();
        assert_eq!(
            actual,
            image::imageops::crop_imm(&full, 20, 30, 100, 80).to_image(),
            "rotation {rotation}"
        );
        let output = directory.path().join(format!("region-{rotation}.png"));
        omapdf::region::write(&output, &encoded).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), encoded);
        assert!(omapdf::region::write(&output, b"overwrite").is_err());
        assert_eq!(std::fs::read(&output).unwrap(), encoded);
        assert!(omapdf::region::write(&pdf, &encoded).is_err());
        assert_eq!(std::fs::read(&pdf).unwrap(), bytes);
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&pdf)
            .unwrap()
            .write_all(b"\n% changed revision\n")
            .unwrap();
        assert!(
            region.render(doc, &renderer, false).is_err(),
            "Stale cached pixels must not be exported after a rewrite"
        );
    }
}

#[test]
fn native_image_regions_support_encrypted_pdf_and_epub_sessions() {
    use mupdf::pdf::{Encryption, PdfDocument, PdfWriteOptions};
    use omapdf::{
        document::Rect,
        raster::Rasterizer,
        region::{Placement, Region},
    };
    use std::sync::Arc;
    let directory = tempfile::tempdir().unwrap();
    let plain = directory.path().join("plain.pdf");
    write_pdf(&plain);
    let locked = directory.path().join("locked.pdf");
    {
        let native = PdfDocument::open(plain.to_str().unwrap()).unwrap();
        let mut options = PdfWriteOptions::default();
        options
            .set_encryption(Encryption::Aes256)
            .set_user_password("image-secret")
            .set_owner_password("image-owner");
        native
            .save_with_options(locked.to_str().unwrap(), options)
            .unwrap();
    }
    let encrypted =
        Document::unlock_preview(&locked, Default::default(), "image-secret".into()).unwrap();
    let epub = Document::open_preview(
        std::path::Path::new("examples/reading-lab.epub"),
        Default::default(),
    )
    .unwrap();
    for document in [encrypted, epub] {
        let size = [document.pages[0].width, document.pages[0].height];
        let pages = [Placement {
            page: 0,
            bounds: Rect {
                x: 0.,
                y: 0.,
                width: size[0],
                height: size[1],
            },
            size,
        }];
        let region = Region::select(
            [20., 20.],
            [120., 120.],
            Rect {
                x: 0.,
                y: 0.,
                width: size[0],
                height: size[1],
            },
            1.5,
            0,
            &pages,
        )
        .unwrap();
        let png = region
            .render(Arc::new(document), &Rasterizer::new(), false)
            .unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (150, 150));
        assert!(image.pixels().all(|pixel| pixel[3] == 255));
    }
}

#[test]
fn text_spans_load_lazy_pages_copy_in_order_and_create_linked_highlights() {
    use omapdf::selection::{self, Endpoint, Span};
    use std::sync::atomic::AtomicBool;
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("two.pdf");
    write_pdf_pages(&pdf, true);
    let doc = Document::open_preview(&pdf, Default::default()).unwrap();
    let span = Span {
        anchor: Endpoint { page: 0, word: 1 },
        focus: Endpoint { page: 1, word: 1 },
    };
    assert!(span.cached(&doc).is_none());
    assert_eq!(span.range(0, 3), Some(1..3));
    assert_eq!(span.range(1, 3), Some(0..2));
    assert_eq!(span.range(2, 3), None);
    let cancel = AtomicBool::new(false);
    let fragments = span.extract(&doc, &cancel).unwrap();
    assert_eq!(
        selection::text(&fragments),
        "research world\nHello research"
    );
    assert_eq!(
        fragments
            .iter()
            .map(|fragment| fragment.page)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert!(fragments.iter().all(|fragment| fragment.rects.len() == 2));
    let backwards = Span {
        anchor: span.focus,
        focus: span.anchor,
    };
    assert_eq!(backwards.extract(&doc, &cancel).unwrap(), fragments);
    let highlights = selection::highlights(fragments, 'B');
    assert_eq!(highlights.len(), 2);
    assert_eq!(highlights[0].meta.group, highlights[1].meta.group);
    assert!(highlights[0].meta.group.is_some());
    assert_ne!(highlights[0].meta.uuid, highlights[1].meta.uuid);
    let mut library = Library::default();
    library.touch(&doc.id, &pdf);
    library.documents.get_mut(&doc.id).unwrap().highlights = highlights;
    let selected = library.documents[&doc.id].highlights[1].meta.clone();
    assert!(library.set_highlight_note(&doc.id, &selected, "One comment"));
    assert!(
        library.documents[&doc.id]
            .highlights
            .iter()
            .all(|highlight| highlight.note.as_deref() == Some("One comment"))
    );
    let (exported, issues) =
        omapdf::interchange::export_value(&library, Default::default()).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        exported["documents"][0]["highlights"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(library.delete_annotation(&selected.uuid));
    assert!(library.documents[&doc.id].highlights.is_empty());
}

#[test]
fn text_spans_handle_blank_pages_invalid_endpoints_cancel_and_changed_sources() {
    use omapdf::{
        document::Rect,
        selection::{self, Endpoint, Fragment, Span},
    };
    use std::sync::atomic::AtomicBool;
    let directory = tempfile::tempdir().unwrap();
    let pdf = directory.path().join("lazy.pdf");
    write_pdf_pages(&pdf, true);
    let doc = Document::open_preview(&pdf, Default::default()).unwrap();
    let span = Span {
        anchor: Endpoint { page: 0, word: 1 },
        focus: Endpoint { page: 1, word: 1 },
    };
    assert!(span.extract(&doc, &AtomicBool::new(true)).is_err());
    assert!(!doc.text_ready(1));
    let blank_span = Span {
        anchor: Endpoint { page: 0, word: 1 },
        focus: Endpoint { page: 2, word: 1 },
    };
    assert_eq!(blank_span.range(1, 0), None);
    assert_eq!(
        selection::text(&[
            Fragment {
                page: 0,
                text: "Before".into(),
                rects: vec![Rect::default()]
            },
            Fragment {
                page: 2,
                text: "After".into(),
                rects: vec![]
            }
        ]),
        "Before\n\nAfter"
    );
    let invalid = Span {
        anchor: Endpoint {
            page: usize::MAX,
            word: 0,
        },
        focus: Endpoint { page: 0, word: 0 },
    };
    assert!(invalid.extract(&doc, &AtomicBool::new(false)).is_err());
    let invalid_word = Span {
        anchor: Endpoint {
            page: 0,
            word: usize::MAX,
        },
        focus: Endpoint { page: 0, word: 0 },
    };
    assert!(invalid_word.extract(&doc, &AtomicBool::new(false)).is_err());
    write_pdf_stream(
        &pdf,
        true,
        "BT /F1 18 Tf 30 150 Td (Changed source words) Tj ET\n",
    );
    assert!(span.extract(&doc, &AtomicBool::new(false)).is_err());
    assert!(!doc.text_ready(1));
}
