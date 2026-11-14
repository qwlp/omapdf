use std::time::Instant;
fn main() -> anyhow::Result<()> {
    let file = std::env::args().nth(1).unwrap();
    let doc = mupdf::Document::open(file.as_str())?;
    let page = doc.load_page(0)?;
    let bounds = page.bounds()?;
    let start = Instant::now();
    let list = page.to_display_list(true)?;
    println!("list {:?}", start.elapsed());
    for width in [1600., 3200.] {
        let start = Instant::now();
        let scale = width / (bounds.x1 - bounds.x0);
        let pix = list.to_pixmap(
            &mupdf::Matrix::new_scale(scale, scale),
            &mupdf::Colorspace::device_bgr(),
            false,
        )?;
        println!(
            "native {width} {:?} {}x{}",
            start.elapsed(),
            pix.width(),
            pix.height()
        );
        let start = Instant::now();
        let mut bytes = Vec::with_capacity(pix.width() as usize * pix.height() as usize * 4);
        for p in pix.samples().as_chunks::<3>().0 {
            bytes.extend_from_slice(&[p[0], p[1], p[2], 255]);
        }
        println!("transfer {:?}", start.elapsed());
        let start = Instant::now();
        use fast_image_resize::{
            FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image,
        };
        let input = Image::from_vec_u8(pix.width(), pix.height(), bytes, PixelType::U8x4)?;
        let mut output = Image::new(
            1600,
            (1600. * (bounds.y1 - bounds.y0) / (bounds.x1 - bounds.x0)).ceil() as u32,
            PixelType::U8x4,
        );
        Resizer::new().resize(
            &input,
            &mut output,
            &ResizeOptions::new()
                .resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
                .use_alpha(false),
        )?;
        println!("resize {:?}", start.elapsed());
    }
    Ok(())
}
