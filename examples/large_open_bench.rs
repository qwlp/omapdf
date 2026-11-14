use omapdf::{
    document::{Document, EpubOptions},
    raster::Rasterizer,
};
use std::{path::PathBuf, sync::Arc, time::Instant};
fn main() -> anyhow::Result<()> {
    let path = PathBuf::from(std::env::args_os().nth(1).expect("document path"));
    let start = Instant::now();
    let doc = Arc::new(Document::open_preview(&path, EpubOptions::default())?);
    println!(
        "{} pages; ready {:.2}ms; {} text pages loaded",
        doc.pages.len(),
        start.elapsed().as_secs_f64() * 1000.,
        doc.indexed_pages()
    );
    let renderer = Rasterizer::new();
    let start = Instant::now();
    renderer.render(doc.clone(), 0, 1200, false)?;
    println!(
        "first raster {:.2}ms",
        start.elapsed().as_secs_f64() * 1000.
    );
    let start = Instant::now();
    doc.ensure_page_text(doc.pages.len() - 1)?;
    println!(
        "last-page text {:.2}ms; {} indexed pages",
        start.elapsed().as_secs_f64() * 1000.,
        doc.indexed_pages()
    );
    let start = Instant::now();
    doc.ensure_all_text()?;
    println!(
        "full text index {:.2}ms",
        start.elapsed().as_secs_f64() * 1000.
    );
    Ok(())
}
