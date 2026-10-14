#[path = "src/icon.rs"]
mod icon;

use std::{env, fs, path::PathBuf, process::Command};

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" name="Tempo.App" type="win32"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security><requestedPrivileges><requestedExecutionLevel level="asInvoker" uiAccess="false"/></requestedPrivileges></security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application><supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/></application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
"#;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out.join("tempo.ico"), icon::ico_file(&[16, 24, 32, 48, 64, 256])).unwrap();
    fs::write(out.join("tempo.manifest"), MANIFEST).unwrap();
    fs::write(
        out.join("tempo.rc"),
        "1 ICON \"tempo.ico\"\n1 24 \"tempo.manifest\"\n\
         1 VERSIONINFO\nFILEVERSION 1,0,0,0\nPRODUCTVERSION 1,0,0,0\nBEGIN\n BLOCK \"StringFileInfo\"\n BEGIN\n  BLOCK \"040904B0\"\n  BEGIN\n   VALUE \"FileDescription\", \"Tempo\"\n   VALUE \"ProductName\", \"Tempo\"\n   VALUE \"FileVersion\", \"1.0.0\"\n  END\n END\n BLOCK \"VarFileInfo\"\n BEGIN\n  VALUE \"Translation\", 0x409, 1200\n END\nEND\n",
    )
    .unwrap();

    let windres = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") && cfg!(not(windows)) {
        "x86_64-w64-mingw32-windres"
    } else {
        "windres"
    };
    let obj = out.join("tempo_res.o");
    let ok = Command::new(windres)
        .current_dir(&out)
        .args(["tempo.rc", "-O", "coff", "-o"])
        .arg(&obj)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        println!("cargo:rustc-link-arg-bins={}", obj.display());
    } else {
        println!("cargo:warning=windres not found, building without icon resource");
    }
}
