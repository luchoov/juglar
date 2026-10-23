//! Embeds the manifest (see `juglar.manifest`), the icon and the version
//! (see `icon/juglar.rc`) into juglar.exe with the MSVC tools.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=juglar.manifest");
    println!("cargo:rerun-if-changed=icon/juglar.rc");
    println!("cargo:rerun-if-changed=icon/juglar.ico");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows && msvc {
        let here = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let manifest = here.join("juglar.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
        match resources(&here) {
            Ok(compiled) => println!("cargo:rustc-link-arg-bins={}", compiled.display()),
            // Without the Windows SDK's resource compiler Juglar still
            // builds, with Windows' generic icon.
            Err(why) => println!("cargo:warning=juglar.exe gets no icon: {why}"),
        }
    }
}

/// Compiles `icon/juglar.rc` with rc.exe into a .res file the linker takes.
fn resources(here: &Path) -> Result<PathBuf, String> {
    let rc = resource_compiler().ok_or("rc.exe not found (the Windows SDK)")?;
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("juglar.res");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts: Vec<&str> = version.split(['.', '-']).take(3).collect();
    parts.resize(3, "0");
    let status = Command::new(&rc)
        .arg("/nologo")
        .arg(format!("/DVERSION={},0", parts.join(",")))
        .arg(format!("/DVERSION_TEXT=\"{version}\""))
        .arg("/fo")
        .arg(&out)
        .arg(here.join("icon").join("juglar.rc"))
        .status()
        .map_err(|e| format!("running {}: {e}", rc.display()))?;
    if status.success() { Ok(out) } else { Err(format!("rc.exe failed ({status})")) }
}

/// rc.exe on the path, or the newest in the Windows 10/11 SDK.
fn resource_compiler() -> Option<PathBuf> {
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return Some("rc.exe".into());
    }
    let kits = PathBuf::from(std::env::var("ProgramFiles(x86)").ok()?).join("Windows Kits").join("10").join("bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(kits).ok()?.flatten().map(|e| e.path()).collect();
    versions.sort();
    versions.into_iter().rev().map(|v| v.join("x64").join("rc.exe")).find(|rc| rc.exists())
}
