//! Pages as PDF: each page is the SVG the Figure tab shows, set as vector PDF
//! with the computer's own fonts embedded, and the project can travel inside
//! the file as an attachment, so opening the PDF gives the work back.

use std::sync::{Arc, OnceLock};

use krilla::Document;
use krilla::embed::{AssociationKind, EmbeddedFile, MimeType};
use krilla::geom::Size;
use krilla::page::PageSettings;
use krilla_svg::{SurfaceExt, SvgSettings};
use usvg::fontdb::Database;

/// What the attached project is called inside the PDF.
pub const ATTACHMENT: &str = "project.opennmr";

/// Points per CSS pixel: usvg measures a page in pixels at 96 to the inch.
const PT_PER_PX: f32 = 72.0 / 96.0;

/// The computer's fonts, read once: looking through every font file takes a
/// moment, and every page and the font list share them.
pub fn fonts() -> Arc<Database> {
    static FONTS: OnceLock<Arc<Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// A PDF of `pages`, one page each, with `project` attached when given.
pub fn write(pages: &[String], project: Option<Vec<u8>>) -> Result<Vec<u8>, String> {
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let mut document = Document::new();
    for (n, svg) in pages.iter().enumerate() {
        let tree = usvg::Tree::from_str(svg, &options)
            .map_err(|e| format!("Page {} could not be read: {e}", n + 1))?;
        let (w, h) = (tree.size().width(), tree.size().height());
        let settings = PageSettings::from_wh(w * PT_PER_PX, h * PT_PER_PX)
            .ok_or(format!("Page {} has no size", n + 1))?;
        let mut page = document.start_page_with(settings);
        let mut surface = page.surface();
        let size = Size::from_wh(w * PT_PER_PX, h * PT_PER_PX).ok_or("A page has no size")?;
        surface
            .draw_svg(&tree, size, SvgSettings::default())
            .ok_or(format!("Page {} could not be drawn", n + 1))?;
        surface.finish();
        page.finish();
    }
    if let Some(bytes) = project {
        document
            .embed_file(EmbeddedFile {
                path: ATTACHMENT.into(),
                mime_type: MimeType::new("application/zip"),
                description: Some("The openNMR project this figure was made from".into()),
                association_kind: AssociationKind::Source,
                data: bytes.into(),
                modification_date: None,
                // Kept as it is, so the project can be found in the file again
                // without a PDF reader: it is a zip, and compressed already.
                compress: Some(false),
                location: None,
            })
            .ok_or("The project could not be attached")?;
    }
    document
        .finish()
        .map_err(|e| format!("The PDF could not be written: {e:?}"))
}

/// The project attached to a PDF written by `write`, if it carries one: the
/// zip stored as it is, from its first local header to the end of its central
/// directory. Each candidate is checked by reading it as a zip holding a
/// project, since those four bytes could turn up elsewhere in a PDF.
pub fn attached(pdf: &[u8]) -> Option<Vec<u8>> {
    const LOCAL: &[u8] = b"PK\x03\x04";
    const END: &[u8] = b"PK\x05\x06";
    let starts = pdf.windows(4).enumerate().filter(|(_, w)| *w == LOCAL);
    for (start, _) in starts {
        let rest = &pdf[start..];
        let Some(end) = rest.windows(4).position(|w| w == END) else {
            continue;
        };
        // The end record is 22 bytes when it carries no comment, as ours do not.
        let zip = &rest[..(end + 22).min(rest.len())];
        let reader = std::io::Cursor::new(zip);
        if zip::ZipArchive::new(reader)
            .is_ok_and(|mut a| a.by_name(crate::project::MANIFEST).is_ok())
        {
            return Some(zip.to_vec());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="82.5mm" height="60mm" viewBox="0 0 82.5 60"><rect width="100%" height="100%" fill="#fff"/><path d="M3 50L80 50" stroke="#000" stroke-width="0.2"/><text x="40" y="57" font-size="2.5" text-anchor="middle">ppm</text></svg>"##;

    #[test]
    fn pages_become_a_pdf_of_that_size() {
        let one = write(&[PAGE.into()], None).unwrap();
        let two = write(&[PAGE.into(), PAGE.into()], None).unwrap();
        assert!(one.starts_with(b"%PDF") && two.starts_with(b"%PDF"));
        assert!(two.len() > one.len());
        assert!(write(&["not a page".into()], None).is_err());
    }

    #[test]
    fn the_project_comes_back_out_of_the_pdf() {
        let dir = std::env::temp_dir().join(format!("opennmr-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("p.opennmr");
        crate::project::write(&file, "{\"format\":1}", &[], &[]).unwrap();
        let project = std::fs::read(&file).unwrap();
        let pdf = write(&[PAGE.into()], Some(project.clone())).unwrap();
        assert_eq!(attached(&pdf), Some(project));
        assert_eq!(attached(&write(&[PAGE.into()], None).unwrap()), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
