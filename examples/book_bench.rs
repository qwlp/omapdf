use omapdf::{document::Document, raster::Rasterizer};
use std::{sync::Arc, time::Instant};
fn main() -> anyhow::Result<()> {
    let path = std::env::args_os().nth(1).expect("book path");
    let start = Instant::now();
    let doc = Arc::new(Document::open(std::path::Path::new(&path))?);
    println!(
        "{} | {} pages | {} chapters | {} links | {} words | open {:.1}ms",
        doc.title,
        doc.pages.len(),
        doc.outline.len(),
        doc.pages.iter().map(|p| p.links.len()).sum::<usize>(),
        doc.pages.iter().map(|p| p.words.len()).sum::<usize>(),
        start.elapsed().as_secs_f64() * 1000.
    );
    let renderer = Rasterizer::new();
    for page in [0, doc.pages.len() / 2, doc.pages.len() - 1] {
        let start = Instant::now();
        let image = renderer.render(doc.clone(), page, 1200, false)?;
        println!(
            "page {} {:?} {:.1}ms",
            page + 1,
            image.size(0),
            start.elapsed().as_secs_f64() * 1000.
        );
    }
    Ok(())
}
