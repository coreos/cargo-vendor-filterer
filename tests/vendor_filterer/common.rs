use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::process::{Command, Output};

use anyhow::bail;
use anyhow::Result;
use camino;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_vendor_filterer::SELF_NAME;

// Return the project root
pub(crate) fn project_root() -> Result<Utf8PathBuf> {
    let mut path = build_root()?;
    while path.exists() && path.is_dir() {
        let found_lock_file = path
            .read_dir_utf8()?
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().eq("Cargo.lock"));
        if found_lock_file {
            return Ok(path);
        }
        path.pop();
    }
    bail!(io::Error::from(io::ErrorKind::NotFound))
}

// Return the root of the executable's build directory
pub(crate) fn build_root() -> Result<Utf8PathBuf> {
    let mut path: Utf8PathBuf = env::current_exe()?.try_into()?;
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    Ok(path)
}

#[derive(Clone, Copy)]
pub(crate) enum VendorFormat {
    Dir,
    Tar,
    TarGz,
    TarZstd,
}

impl fmt::Display for VendorFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                VendorFormat::Dir => "dir",
                VendorFormat::Tar => "tar",
                VendorFormat::TarGz => "tar.gz",
                VendorFormat::TarZstd => "tar.zstd",
            }
        )
    }
}

pub(crate) struct VendorOptions<'a, 'b, 'c, 'd> {
    pub output: Option<&'a Utf8Path>,
    pub platforms: Option<&'b [&'b str]>,
    pub tier: Option<&'static str>,
    pub exclude_crate_paths: Option<&'c [&'c str]>,
    pub format: Option<VendorFormat>,
    pub manifest_path: Option<&'d Utf8Path>,
}

impl<'a, 'b, 'c, 'd> Default for VendorOptions<'a, 'b, 'c, 'd> {
    fn default() -> Self {
        Self {
            output: None,
            platforms: None,
            tier: None,
            exclude_crate_paths: None,
            format: None,
            manifest_path: None,
        }
    }
}

/// Run a vendoring process
pub(crate) fn vendor(options: VendorOptions) -> Result<Output> {
    use once_cell::sync::OnceCell;
    use std::sync::Mutex;
    // Ensure we only run a vendoring process one at a time to avoid
    // excessive CPU usage.
    static LOCK: OnceCell<Mutex<()>> = OnceCell::new();

    let mut program = build_root()?;
    program.push(format!("cargo-{SELF_NAME}"));
    let mut cmd = Command::new(&program);
    cmd.current_dir(project_root()?).arg(SELF_NAME);
    if let Some(platforms) = options.platforms {
        cmd.args(platforms.iter().map(|&p| format!("--platform={p}")));
    }
    if let Some(tier) = options.tier {
        cmd.args(["--tier", tier]);
    }
    if let Some(exclude_crate_paths) = options.exclude_crate_paths {
        cmd.args(
            exclude_crate_paths
                .iter()
                .map(|&p| format!("--exclude-crate-path={p}")),
        );
    }
    if let Some(format) = options.format {
        cmd.arg(format!("--format={format}"));
    }
    if let Some(manifest_path) = options.manifest_path {
        cmd.arg(format!("--manifest-path={manifest_path}"));
    }
    if let Some(output) = options.output {
        cmd.arg(output);
    }

    Ok({
        let mutex = LOCK.get_or_init(|| Mutex::new(()));
        #[allow(unused)]
        let guard = mutex.lock().unwrap();
        println!("{:?}", cmd.get_args());
        let output = cmd.output()?;
        // io::stdout().write_all(&output.stdout)?;
        // io::stderr().write_all(&output.stderr)?;
        // use std::{thread, time};
        // thread::sleep(time::Duration::from_millis(200));
        output
    })
}

/// Allocate a temporary directory and also gather its UTF-8 path.
pub(crate) fn tempdir() -> Result<(tempfile::TempDir, Utf8PathBuf)> {
    let td = tempfile::tempdir()?;
    let path = Utf8Path::from_path(td.path()).unwrap();
    let path = path.to_owned();
    Ok((td, path))
}

pub(crate) fn write_file_create_parents(
    dir: &Utf8Path,
    path: &str,
    contents: &str,
) -> Result<Utf8PathBuf> {
    let path = dir.join(path);
    println!("writing {path}");
    fs::create_dir_all(
        path.parent()
            .ok_or(io::Error::from(io::ErrorKind::NotFound))?,
    )?;
    std::fs::write(&path, contents.as_bytes())?;
    Ok(path)
}

pub(crate) fn verify_no_windows(dir: &Utf8Path) {
    let mut windows_lib = dir.join("windows-sys/src/lib.rs");
    assert!(windows_lib.exists());
    assert_eq!(windows_lib.metadata().unwrap().len(), 0);

    // check that only one file exists
    windows_lib.pop();
    assert_eq!(windows_lib.read_dir_utf8().unwrap().count(), 1);
}
