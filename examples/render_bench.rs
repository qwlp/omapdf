//! Measure native document opening, native-resolution rendering and warm raster cache hits.
use omapdf::{document::Document, raster::Rasterizer};
use std::{path::PathBuf, sync::Arc, time::Instant};
fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("PDF or EPUB path required");
    let start = Instant::now();
    let doc = Arc::new(Document::open(&path)?);
    println!(
        "{}: {} pages; open/extract {:.2}ms",
        doc.title,
        doc.pages.len(),
        start.elapsed().as_secs_f64() * 1000.
    );
    let renderer = Rasterizer::new();
    let count = doc.pages.len().min(10);
    let mut uncached = 0.;
    let mut cached = 0.;
    for page in 0..count {
        let start = Instant::now();
        renderer.render(doc.clone(), page, 1600, false)?;
        uncached += start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        renderer.render(doc.clone(), page, 1600, false)?;
        cached += start.elapsed().as_secs_f64() * 1000.;
    }
    println!(
        "{} pages at 1600 physical output pixels, native rasterization; uncached mean {:.2}ms; cache mean {:.3}ms",
        count,
        uncached / count as f64,
        cached / count as f64
    );
    Ok(())
}
