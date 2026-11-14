//! Native PDF annotation interchange. Source files are never modified.
use crate::{
    document::{Document, Rect},
    state::{AnnotationMeta, Bookmark, Highlight, Library, Location},
};
use anyhow::{Context, Result, ensure};
use mupdf::{
    color::AnnotationColor,
    pdf::{PdfAnnotationType, PdfDocument, PdfPage},
};
use std::path::Path;

#[derive(Default)]
pub struct Imported {
    pub highlights: Vec<Highlight>,
    pub bookmarks: Vec<Bookmark>,
    pub unsupported: usize,
    highlight_xrefs: Vec<i32>,
    bookmark_xrefs: Vec<i32>,
}

fn color_rgb(color: AnnotationColor) -> [f32; 3] {
    match color {
        AnnotationColor::Gray(gray) => [gray; 3],
        AnnotationColor::Rgb { red, green, blue } => [red, green, blue],
        AnnotationColor::Cmyk {
            cyan,
            magenta,
            yellow,
            key,
        } => [
            (1. - cyan) * (1. - key),
            (1. - magenta) * (1. - key),
            (1. - yellow) * (1. - key),
        ],
    }
}
fn palette_rgb(color: u32) -> [f32; 3] {
    [
        (color >> 24) as u8 as f32 / 255.,
        (color >> 16) as u8 as f32 / 255.,
        (color >> 8) as u8 as f32 / 255.,
    ]
}
fn closest_kind(color: Option<AnnotationColor>, colors: &[u32; 26]) -> char {
    let color = color.map(color_rgb).unwrap_or([1., 1., 0.]);
    let index = colors
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let distance = |value| {
                color
                    .iter()
                    .zip(palette_rgb(value))
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
            };
            distance(**a).total_cmp(&distance(**b))
        })
        .unwrap()
        .0;
    (b'a' + index as u8) as char
}
fn local_rect(rect: mupdf::Rect, origin: mupdf::Rect) -> Rect {
    Rect {
        x: rect.x0 - origin.x0,
        y: rect.y0 - origin.y0,
        width: rect.x1 - rect.x0,
        height: rect.y1 - rect.y0,
    }
}
fn close_rect(a: Rect, b: Rect) -> bool {
    [a.x - b.x, a.y - b.y, a.width - b.width, a.height - b.height]
        .iter()
        .all(|delta| delta.abs() <= 0.15)
}
fn same_highlight(a: &Highlight, b: &Highlight) -> bool {
    if a.page != b.page || a.kind != b.kind {
        return false;
    }
    let a = crate::highlight::line_boxes(&a.rects);
    let b = crate::highlight::line_boxes(&b.rects);
    a.len() == b.len() && a.iter().all(|a| b.iter().any(|b| close_rect(*a, *b)))
}
fn same_bookmark(a: &Bookmark, b: &Bookmark) -> bool {
    a.name == b.name
        && a.location.document == b.location.document
        && a.location.page == b.location.page
        && (a.location.offset[1] - b.location.offset[1]).abs() < 0.02
}

fn annotation_meta(
    annotation: &mupdf::pdf::PdfAnnotation,
    doc: &Document,
    page: usize,
) -> Result<AnnotationMeta> {
    let object = annotation.object();
    let field = |key| -> Result<String> {
        Ok(object
            .get_dict(key)?
            .and_then(|value| value.as_string().ok())
            .unwrap_or_default())
    };
    let date = |value: String| {
        let digits = value
            .strip_prefix("D:")
            .unwrap_or(&value)
            .get(..14)
            .unwrap_or("");
        chrono::NaiveDateTime::parse_from_str(digits, "%Y%m%d%H%M%S")
            .ok()
            .map(|date| date.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default()
    };
    let name = field("NM")?;
    let owner = field("OmapdfUUID")?;
    let uuid = if !owner.is_empty() {
        owner.clone()
    } else {
        let identity = if !name.is_empty() {
            serde_json::json!([doc.path, page, name])
        } else {
            serde_json::json!([
                doc.path,
                page,
                annotation.xref()?,
                format!("{:?}", annotation.r#type()?),
                annotation
                    .bounds()
                    .ok()
                    .map(|bounds| [bounds.x0, bounds.y0, bounds.x1, bounds.y1])
            ])
        };
        AnnotationMeta::stable("pdf", &identity).uuid
    };
    let raw = serde_json::json!({"uuid":uuid,"pdf_name":if !name.is_empty() && name!=owner {Some(name)} else {None},
        "creation_time":date(field("CreationDate")?),"modification_time":date(field("M")?),
        "group":field("OmapdfGroup")?,
        "revision":field("OmapdfRevision")?.parse::<u64>().unwrap_or_default(),
        "aliases":serde_json::from_str::<Vec<String>>(&field("OmapdfAliases")?).unwrap_or_default()});
    Ok(AnnotationMeta::imported("pdf", &raw))
}

fn write_meta(
    annotation: &mupdf::pdf::PdfAnnotation,
    native: &PdfDocument,
    meta: &AnnotationMeta,
    fallback: &serde_json::Value,
) -> Result<()> {
    let uuid = if meta.uuid.is_empty() {
        AnnotationMeta::stable("pdf-export", fallback).uuid
    } else {
        meta.uuid.clone()
    };
    let mut object = annotation.object();
    object.dict_put(
        "NM",
        native.new_string(meta.pdf_name.as_deref().unwrap_or(&uuid))?,
    )?;
    object.dict_put("OmapdfUUID", native.new_string(&uuid)?)?;
    object.dict_put(
        "OmapdfGroup",
        native.new_string(meta.group.as_deref().unwrap_or(""))?,
    )?;
    object.dict_put(
        "OmapdfAliases",
        native.new_string(&serde_json::to_string(&meta.aliases)?)?,
    )?;
    object.dict_put(
        "OmapdfRevision",
        native.new_string(&meta.revision.to_string())?,
    )?;
    for (key, date) in [
        ("CreationDate", &meta.creation_time),
        ("M", &meta.modification_time),
    ] {
        if let Ok(date) = chrono::NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S") {
            object.dict_put(
                key,
                native.new_string(&date.format("D:%Y%m%d%H%M%S").to_string())?,
            )?;
        }
    }
    Ok(())
}

fn read_native(doc: &Document, native: &PdfDocument, colors: &[u32; 26]) -> Result<Imported> {
    let mut result = Imported::default();
    for index in 0..doc.pages.len() {
        let page = PdfPage::try_from(native.load_page(index as i32)?)?;
        let origin = page.bounds()?;
        let mut words = None;
        for annotation in page.annotations() {
            match annotation.r#type()? {
                style @ (PdfAnnotationType::Highlight
                | PdfAnnotationType::Underline
                | PdfAnnotationType::StrikeOut) => {
                    let rects: Vec<_> = annotation
                        .quad_points()?
                        .into_iter()
                        .map(|quad| local_rect(quad.into(), origin))
                        .collect();
                    if rects.is_empty() {
                        continue;
                    }
                    if words.is_none() {
                        words = Some(page.words(Default::default())?);
                    }
                    let selected = words
                        .as_ref()
                        .unwrap()
                        .iter()
                        .filter(|word| {
                            let r = local_rect(word.bounds, origin);
                            rects.iter().any(|area| {
                                let x = r.x + r.width / 2.;
                                let y = r.y + r.height / 2.;
                                x >= area.x - 0.5
                                    && x <= area.x + area.width + 0.5
                                    && y >= area.y - 0.5
                                    && y <= area.y + area.height + 0.5
                            })
                        })
                        .map(|word| word.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let contents = annotation.contents()?.unwrap_or("").to_owned();
                    let kind = annotation
                        .object()
                        .get_dict("OmapdfHighlightType")?
                        .and_then(|object| object.as_string().ok())
                        .and_then(|value| value.chars().next())
                        .filter(|kind| kind.is_ascii_alphabetic() || *kind == '_')
                        .unwrap_or(match style {
                            PdfAnnotationType::Underline => {
                                closest_kind(annotation.color()?, colors).to_ascii_uppercase()
                            }
                            PdfAnnotationType::StrikeOut => '_',
                            _ => closest_kind(annotation.color()?, colors),
                        });
                    result.highlight_xrefs.push(annotation.xref()?);
                    result.highlights.push(Highlight {
                        meta: annotation_meta(&annotation, doc, index)?,
                        page: index,
                        kind,
                        rects,
                        text: if selected.is_empty() {
                            contents.clone()
                        } else {
                            selected
                        },
                        note: (!contents.is_empty()).then_some(contents),
                    });
                }
                PdfAnnotationType::Text | PdfAnnotationType::FreeText => {
                    let name = annotation.contents()?.unwrap_or("").to_owned();
                    if name.is_empty() {
                        continue;
                    }
                    let rect = local_rect(annotation.bounds()?, origin);
                    result.bookmark_xrefs.push(annotation.xref()?);
                    result.bookmarks.push(Bookmark {
                        meta: annotation_meta(&annotation, doc, index)?,
                        name,
                        location: Location {
                            document: doc.id.clone(),
                            page: index,
                            zoom: 1.,
                            fit: true,
                            offset: [
                                -rect.x / doc.pages[index].width,
                                -rect.y / doc.pages[index].width,
                            ],
                        },
                    });
                }
                PdfAnnotationType::Link | PdfAnnotationType::Popup | PdfAnnotationType::Widget => {}
                _ => result.unsupported += 1,
            }
        }
    }
    Ok(result)
}

pub fn read(doc: &Document, colors: &[u32; 26]) -> Result<Imported> {
    ensure!(
        !doc.reflowable,
        "PDF annotation import is available for PDF documents"
    );
    let _access = crate::document::native_access(doc.epub_options)?;
    let mut native = PdfDocument::open(doc.path.to_str().context("PDF path must be UTF-8")?)?;
    doc.authenticate_native(&mut native)?;
    let imported = read_native(doc, &native, colors)?;
    ensure!(
        crate::reload::Revision::read(&doc.path)? == doc.revision,
        "Document changed during annotation import; retry after reload"
    );
    Ok(imported)
}

pub fn merge(library: &mut Library, id: &str, imported: Imported) -> Result<(usize, usize, usize)> {
    let mut imported = imported;
    imported
        .highlights
        .retain(|item| !library.is_deleted(&item.meta));
    imported
        .bookmarks
        .retain(|item| !library.is_deleted(&item.meta));
    let state = library
        .documents
        .get_mut(id)
        .context("Document is no longer in the library")?;
    let mut highlights = 0;
    for highlight in imported.highlights {
        if let Some(existing) = state.highlights.iter_mut().find(|existing| {
            existing.meta.same_identity(&highlight.meta)
                || (same_highlight(existing, &highlight) && existing.note == highlight.note)
        }) {
            existing.meta.absorb_identity(&highlight.meta);
        } else {
            state.highlights.push(highlight);
            highlights += 1;
        }
    }
    let mut bookmarks = 0;
    for bookmark in imported.bookmarks {
        if let Some(existing) = library.bookmarks.iter_mut().find(|existing| {
            existing.meta.same_identity(&bookmark.meta) || same_bookmark(existing, &bookmark)
        }) {
            existing.meta.absorb_identity(&bookmark.meta);
        } else {
            library.bookmarks.push(bookmark);
            bookmarks += 1;
        }
    }
    Ok((highlights, bookmarks, imported.unsupported))
}

/// Write a new PDF atomically. Refuse existing paths, including the source and symlinks.
pub fn embed(
    doc: &Document,
    library: &Library,
    colors: &[u32; 26],
    destination: &Path,
) -> Result<(usize, usize)> {
    ensure!(
        !doc.reflowable,
        "Embedding annotations is available for PDF documents"
    );
    ensure!(
        !destination.try_exists()?,
        "Output already exists; choose a new PDF filename"
    );
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = tempfile::Builder::new()
        .suffix(".pdf")
        .tempfile_in(parent)?;
    let _access = crate::document::native_access(doc.epub_options)?;
    let mut native = PdfDocument::open(doc.path.to_str().context("PDF path must be UTF-8")?)?;
    doc.authenticate_native(&mut native)?;
    // Remove deleted annotations in the output document's memory only.
    // The original PDF and unrelated embedded annotations remain unchanged.
    if !library.deleted_annotations.is_empty() {
        for index in 0..doc.pages.len() {
            let mut page = PdfPage::try_from(native.load_page(index as i32)?)?;
            let mut removed = Vec::new();
            for annotation in page.annotations() {
                if library.is_deleted(&annotation_meta(&annotation, doc, index)?) {
                    removed.push(annotation);
                }
            }
            if !removed.is_empty() {
                for annotation in removed {
                    page.delete_annotation(annotation)?;
                }
                page.update()?;
            }
        }
    }
    let existing = read_native(doc, &native, colors)?;
    let state = library
        .documents
        .get(&doc.id)
        .context("No annotations for this document")?;
    let mut highlights = 0;
    for highlight in &state.highlights {
        let matches: Vec<_> = existing
            .highlights
            .iter()
            .enumerate()
            .filter(|(_, old)| same_highlight(old, highlight))
            .collect();
        let index = existing
            .highlights
            .iter()
            .position(|old| old.meta.same_identity(&highlight.meta))
            .or_else(|| {
                matches
                    .iter()
                    .find(|(_, old)| old.note == highlight.note)
                    .map(|(index, _)| *index)
            })
            .or_else(|| {
                if matches.len() == 1 {
                    Some(matches[0].0)
                } else {
                    None
                }
            });
        ensure!(
            index.is_some() || matches.is_empty(),
            "Overlapping embedded highlights cannot be identified uniquely; import their annotation identities before editing"
        );
        if let Some(index) = index {
            let desired = highlight.note.as_deref().unwrap_or("");
            let changed = existing.highlights[index].note.as_deref().unwrap_or("") != desired;
            let mut page = PdfPage::try_from(native.load_page(highlight.page as i32)?)?;
            for mut annotation in page.annotations() {
                if annotation.xref()? == existing.highlight_xrefs[index] {
                    if changed {
                        annotation.set_contents(desired)?;
                    }
                    write_meta(
                        &annotation,
                        &native,
                        &highlight.meta,
                        &serde_json::to_value(highlight)?,
                    )?;
                    annotation.update()?;
                    break;
                }
            }
            page.update()?;
            highlights += usize::from(changed);
            continue;
        }
        let dimensions = doc
            .pages
            .get(highlight.page)
            .context("Highlight page is out of range")?;
        let rects = crate::highlight::line_boxes(&highlight.rects);
        ensure!(!rects.is_empty(), "Highlight has no valid geometry");
        let mut page = PdfPage::try_from(native.load_page(highlight.page as i32)?)?;
        let origin = page.bounds()?;
        let quads: Vec<mupdf::Quad> = rects
            .iter()
            .map(|rect| {
                mupdf::Rect::new(
                    rect.x + origin.x0,
                    rect.y + origin.y0,
                    rect.x + rect.width + origin.x0,
                    rect.y + rect.height + origin.y0,
                )
                .into()
            })
            .collect();
        ensure!(
            rects.iter().all(|r| r.x >= -1.
                && r.y >= -1.
                && r.x + r.width <= dimensions.width + 1.
                && r.y + r.height <= dimensions.height + 1.),
            "Highlight extends outside its PDF page"
        );
        let mut annotation = if highlight.kind == '_' {
            page.add_strikeout_annotation(quads)?
        } else if highlight.kind.is_ascii_uppercase() {
            page.add_underline_annotation(quads)?
        } else {
            page.add_highlight_annotation(quads)?
        };
        let index = (highlight.kind.to_ascii_lowercase() as usize)
            .saturating_sub('a' as usize)
            .min(25);
        let [red, green, blue] = palette_rgb(colors[index]);
        annotation.set_color(AnnotationColor::Rgb { red, green, blue })?;
        annotation.set_opacity(0.35)?;
        annotation.set_contents(highlight.note.as_deref().unwrap_or(""))?;
        annotation.set_author("Omapdf")?;
        write_meta(
            &annotation,
            &native,
            &highlight.meta,
            &serde_json::to_value(highlight)?,
        )?;
        annotation.object().dict_put(
            "OmapdfHighlightType",
            native.new_string(&highlight.kind.to_string())?,
        )?;
        annotation.update()?;
        page.update()?;
        highlights += 1;
    }
    let mut bookmarks = 0;
    for bookmark in library
        .bookmarks
        .iter()
        .filter(|b| b.location.document == doc.id)
    {
        if let Some(index) = existing
            .bookmarks
            .iter()
            .position(|old| old.meta.same_identity(&bookmark.meta) || same_bookmark(old, bookmark))
        {
            let changed = existing.bookmarks[index].name != bookmark.name;
            let mut page = PdfPage::try_from(native.load_page(bookmark.location.page as i32)?)?;
            for mut annotation in page.annotations() {
                if annotation.xref()? == existing.bookmark_xrefs[index] {
                    if changed {
                        annotation.set_contents(&bookmark.name)?;
                    }
                    write_meta(
                        &annotation,
                        &native,
                        &bookmark.meta,
                        &serde_json::to_value(bookmark)?,
                    )?;
                    annotation.update()?;
                    break;
                }
            }
            page.update()?;
            bookmarks += usize::from(changed);
            continue;
        }
        let dimensions = doc
            .pages
            .get(bookmark.location.page)
            .context("Bookmark page is out of range")?;
        let mut page = PdfPage::try_from(native.load_page(bookmark.location.page as i32)?)?;
        let origin = page.bounds()?;
        let x = (-bookmark.location.offset[0] * dimensions.width)
            .clamp(0., (dimensions.width - 18.).max(0.))
            + origin.x0;
        let y = (-bookmark.location.offset[1] * dimensions.width)
            .clamp(0., (dimensions.height - 18.).max(0.))
            + origin.y0;
        let mut annotation = page.create_annotation(PdfAnnotationType::Text)?;
        annotation.set_rect(mupdf::Rect::new(x, y, x + 18., y + 18.))?;
        annotation.set_contents(&bookmark.name)?;
        annotation.set_author("Omapdf")?;
        write_meta(
            &annotation,
            &native,
            &bookmark.meta,
            &serde_json::to_value(bookmark)?,
        )?;
        annotation.update()?;
        page.update()?;
        bookmarks += 1;
    }
    ensure!(
        crate::reload::Revision::read(&doc.path)? == doc.revision,
        "Document changed during export; retry after reload"
    );
    native.save(
        temporary
            .path()
            .to_str()
            .context("Output path must be UTF-8")?,
    )?;
    ensure!(
        crate::reload::Revision::read(&doc.path)? == doc.revision,
        "Document changed while saving annotations; retry after reload"
    );
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(destination)
        .map_err(|error| error.error)?;
    Ok((highlights, bookmarks))
}
