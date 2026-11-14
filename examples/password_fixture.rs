//! Generate isolated encrypted PDFs for native and Wayland regression tests.
use anyhow::Result;
use mupdf::pdf::{Encryption, PdfDocument, PdfWriteOptions};
use std::path::Path;
fn main() -> Result<()> {
    let directory = std::env::args_os().nth(1).expect("fixture directory");
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    let stream = "BT /F1 18 Tf 30 150 Td (Password research fixture) Tj ET\n";
    let mut objects = vec!["<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
        format!("<< /Length {} >>\nstream\n{stream}endstream",stream.len())];
    objects.push(objects[2].clone());
    let mut pdf = "%PDF-1.4\n".to_owned();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf += &format!("{} 0 obj\n{object}\nendobj\n", index + 1);
    }
    let xref = pdf.len();
    pdf += &format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        pdf += &format!("{offset:010} 00000 n \n");
    }
    pdf += &format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );
    let plain = directory.join("plain.pdf");
    std::fs::write(&plain, pdf)?;
    for (name, password) in [
        ("locked.pdf", "reader-secret"),
        ("revised.pdf", "reader-secret"),
        ("rekeyed.pdf", "replacement-secret"),
    ] {
        let native = PdfDocument::open(plain.to_str().unwrap())?;
        let mut options = PdfWriteOptions::default();
        options
            .set_encryption(Encryption::Aes256)
            .set_user_password(password)
            .set_owner_password("owner-secret");
        native.save_with_options(directory.join(name).to_str().unwrap(), options)?;
    }
    Ok(())
}
