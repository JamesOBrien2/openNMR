//! The `.opennmr` file: a zip holding a readable `project.json` and, for each
//! spectrum, the Bruker files it was read from and its structure, so a project
//! opens on any computer. A path to the data was all a project used to hold, and
//! it opened nowhere else. Any unzip tool shows what is inside.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const MANIFEST: &str = "project.json";

/// What a Bruker experiment folder holds that the app reads. A folder often
/// holds other things - exports, other programs' files - that are not the data
/// and are left behind.
const EXPERIMENT_FILES: &[&str] = &[
    "fid",
    "ser",
    "acqu",
    "acqus",
    "acqu2",
    "acqu2s",
    "pulseprogram",
    "audita.txt",
];
/// The same for each `pdata/N`: parameters, the processed spectrum and title.
const PROCESSED_FILES: &[&str] = &[
    "proc",
    "procs",
    "proc2",
    "proc2s",
    "1r",
    "1i",
    "title",
    "auditp.txt",
];

/// The experiment folder a document was read from: itself, or two up from the
/// `pdata/N` folder of the processed spectrum.
fn experiment(path: &Path) -> &Path {
    if path.join("1r").is_file() {
        path.parent().and_then(Path::parent).unwrap_or(path)
    } else {
        path
    }
}

fn file_name(path: Option<&Path>, or: &str) -> String {
    path.and_then(Path::file_name)
        .map_or_else(|| or.to_string(), |n| n.to_string_lossy().into_owned())
}

/// Where the experiment of document `k` goes in the archive. The sample and
/// experiment folder names are kept, since a spectrum is named after them.
fn prefix(k: usize, exp: &Path) -> String {
    format!(
        "spectra/{k}/{}/{}",
        file_name(exp.parent(), "sample"),
        file_name(Some(exp), "1")
    )
}

/// Where document `k`, read from `path`, is found inside the archive.
pub fn inner_path(k: usize, path: &Path) -> String {
    let exp = experiment(path);
    let mut inner = prefix(k, exp);
    if let Ok(rest) = path.strip_prefix(exp) {
        for part in rest.components() {
            inner.push('/');
            inner.push_str(&part.as_os_str().to_string_lossy());
        }
    }
    inner
}

/// The files of an experiment worth keeping, each with its place relative to
/// the experiment folder.
fn files_of(exp: &Path) -> Vec<(PathBuf, String)> {
    let mut files: Vec<(PathBuf, String)> = EXPERIMENT_FILES
        .iter()
        .map(|name| (exp.join(name), (*name).to_string()))
        .filter(|(path, _)| path.is_file())
        .collect();
    let mut processed: Vec<PathBuf> = fs::read_dir(exp.join("pdata"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    processed.sort();
    for dir in processed {
        let n = file_name(Some(&dir), "1");
        for name in PROCESSED_FILES {
            let path = dir.join(name);
            if path.is_file() {
                files.push((path, format!("pdata/{n}/{name}")));
            }
        }
    }
    files
}

/// Writes the archive: `manifest` as `project.json`, the data each of `sources`
/// was read from, and `extras` (a structure drawing, by its place in the
/// archive). Written beside `path` and moved over it, so a save that fails
/// leaves the file there was.
pub fn write(
    path: &Path,
    manifest: &str,
    sources: &[PathBuf],
    extras: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let partial = path.with_extension("opennmr-partial");
    let written = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut zip = ZipWriter::new(File::create(&partial)?);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(MANIFEST, options)?;
        zip.write_all(manifest.as_bytes())?;
        for (k, source) in sources.iter().enumerate() {
            let exp = experiment(source);
            let prefix = prefix(k, exp);
            for (file, rel) in files_of(exp) {
                zip.start_file(format!("{prefix}/{rel}"), options)?;
                io::copy(&mut File::open(&file)?, &mut zip)?;
            }
        }
        for (name, bytes) in extras {
            zip.start_file(name.as_str(), options)?;
            zip.write_all(bytes)?;
        }
        zip.finish()?;
        Ok(())
    })();
    match written {
        Ok(()) => {
            fs::rename(&partial, path).map_err(|e| format!("Cannot write {}: {e}", path.display()))
        }
        Err(e) => {
            let _ = fs::remove_file(&partial);
            Err(format!("Cannot write {}: {e}", path.display()))
        }
    }
}

/// A project as read: its `project.json`, and where its data was unpacked, or
/// none for a project written before the archive, whose paths are on disk.
pub struct Opened {
    pub manifest: String,
    pub root: Option<PathBuf>,
}

/// Reads a project, unpacking an archive into `into`. Only plain files are
/// unpacked, and only under `into`: a project can come from anyone, and a link
/// or a name with `..` in it would write elsewhere.
pub fn open(path: &Path, into: &Path) -> Result<Opened, String> {
    let fail = |e: &dyn std::fmt::Display| format!("Cannot read {}: {e}", path.display());
    let mut file = File::open(path).map_err(|e| fail(&e))?;
    let mut magic = [0u8; 4];
    let is_zip = file.read_exact(&mut magic).is_ok() && magic == *b"PK\x03\x04";
    if !is_zip {
        let manifest = fs::read_to_string(path).map_err(|e| fail(&e))?;
        return Ok(Opened {
            manifest,
            root: None,
        });
    }
    let mut archive =
        ZipArchive::new(File::open(path).map_err(|e| fail(&e))?).map_err(|e| fail(&e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| fail(&e))?;
        let Some(name) = entry.enclosed_name().filter(|_| entry.is_file()) else {
            continue;
        };
        let out = into.join(name);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| fail(&e))?;
        }
        io::copy(&mut entry, &mut File::create(&out).map_err(|e| fail(&e))?)
            .map_err(|e| fail(&e))?;
    }
    let manifest = fs::read_to_string(into.join(MANIFEST))
        .map_err(|_| format!("{} is not an openNMR project", path.display()))?;
    Ok(Opened {
        manifest,
        root: Some(into.to_path_buf()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("opennmr-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_project_carries_its_data_and_nothing_else() {
        let dir = scratch("roundtrip");
        let exp = dir.join("sample/1");
        fs::create_dir_all(exp.join("pdata/1")).unwrap();
        fs::write(exp.join("acqus"), "##TITLE=acqus").unwrap();
        fs::write(exp.join("fid"), [1u8, 2, 3, 4]).unwrap();
        fs::write(exp.join("pdata/1/procs"), "##TITLE=procs").unwrap();
        fs::write(exp.join("pdata/1/1r"), [5u8; 8]).unwrap();
        fs::write(exp.join("export.csv"), "not the data").unwrap();

        let file = dir.join("p.opennmr");
        let sources = [exp.clone(), exp.join("pdata/1")];
        let extras = [("spectra/0/structure.png".to_string(), vec![9u8, 9])];
        write(&file, "{\"format\":1}", &sources, &extras).unwrap();

        let opened = open(&file, &dir.join("unpacked")).unwrap();
        assert_eq!(opened.manifest, "{\"format\":1}");
        let root = opened.root.unwrap();
        // Each document is found where `inner_path` says, with its files.
        let fid = root.join(inner_path(0, &exp));
        assert_eq!(fs::read(fid.join("fid")).unwrap(), [1, 2, 3, 4]);
        assert!(fid.join("pdata/1/procs").is_file());
        assert!(!fid.join("export.csv").exists());
        let processed = root.join(inner_path(1, &sources[1]));
        assert!(processed.ends_with("spectra/1/sample/1/pdata/1"));
        assert!(processed.join("1r").is_file());
        assert_eq!(
            fs::read(root.join("spectra/0/structure.png")).unwrap(),
            [9, 9]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_real_dataset_reads_the_same_out_of_its_project() {
        let exp =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../testdata/bruker/L1C-Cl_00/1");
        if !exp.is_dir() {
            return;
        }
        let dir = scratch("real");
        let file = dir.join("real.opennmr");
        write(&file, "{}", std::slice::from_ref(&exp), &[]).unwrap();
        let root = open(&file, &dir.join("unpacked")).unwrap().root.unwrap();
        let copy = root.join(inner_path(0, &exp));
        let (a, b) = (
            nmr_core::bruker::read_fid(&exp).unwrap(),
            nmr_core::bruker::read_fid(&copy).unwrap(),
        );
        assert_eq!(a.data, b.data);
        assert_eq!(a.meta, b.meta);
        assert_eq!(
            nmr_core::bruker::describe(&exp).unwrap(),
            nmr_core::bruker::describe(&copy).unwrap()
        );
        // The exports kept beside the data stay behind: a quarter of the size.
        assert!(fs::metadata(&file).unwrap().len() < 1_000_000);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_project_written_before_the_archive_is_read_as_it_is() {
        let dir = scratch("plain");
        let file = dir.join("old.opennmr");
        fs::write(&file, "{\"documents\":[]}").unwrap();
        let opened = open(&file, &dir.join("unpacked")).unwrap();
        assert_eq!(opened.manifest, "{\"documents\":[]}");
        assert!(opened.root.is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nothing_is_unpacked_outside_its_folder() {
        let dir = scratch("escape");
        let file = dir.join("bad.opennmr");
        let mut zip = ZipWriter::new(File::create(&file).unwrap());
        let options = SimpleFileOptions::default();
        zip.start_file(MANIFEST, options).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.start_file("../escaped.txt", options).unwrap();
        zip.write_all(b"out").unwrap();
        zip.finish().unwrap();
        open(&file, &dir.join("unpacked")).unwrap();
        assert!(!dir.join("escaped.txt").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
