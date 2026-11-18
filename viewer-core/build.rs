use std::path::{Path, PathBuf};
use std::{env, fs};

/// Copies `native\*.dll` next to the binaries (profile dir) and the test executables (`deps\`),
/// where the libheif loader looks for them.
fn main() {
    let native = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("..").join("native");
    println!("cargo:rerun-if-changed={}", native.display());
    // OUT_DIR is <target>\<profile>\build\<pkg>-<hash>\out
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let profile = out.ancestors().nth(3).expect("OUT_DIR layout");
    for dir in [profile.to_path_buf(), profile.join("deps")] {
        fs::create_dir_all(&dir).unwrap();
        for entry in fs::read_dir(&native).expect("read native dir") {
            let src = entry.unwrap().path();
            if src.extension().is_some_and(|e| e.eq_ignore_ascii_case("dll")) {
                fs::copy(&src, dir.join(src.file_name().unwrap())).unwrap_or_else(|e| panic!("copy {}: {e}", src.display()));
            }
        }
    }
}
