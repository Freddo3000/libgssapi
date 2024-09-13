use std::{env, path::PathBuf, process::Command};

fn search_pat(base: &str, pat: &str) -> bool {
    let res = Command::new("find")
        .arg(base)
        .arg("-name")
        .arg(pat)
        .output();
    match dbg!(res) {
        Err(_) => false,
        Ok(output) => output.stdout.len() > 0,
    }
}

enum Gssapi {
    Mit,
    Heimdal,
    Apple,
}

fn builder_from_pkgconfig(lib: pkg_config::Library) -> bindgen::Builder {
    bindgen::Builder::default().clang_args(
        lib.include_paths
            .iter()
            .map(|path| format!("-I{}", path.to_string_lossy())),
    )
}

fn try_pkgconfig() -> Result<(Gssapi, bindgen::Builder), pkg_config::Error> {
    match pkg_config::probe_library("mit-krb5-gssapi") {
        Ok(lib) => Ok((Gssapi::Mit, builder_from_pkgconfig(lib))),
        Err(_) => match pkg_config::probe_library("heimdal-gssapi") {
            Ok(lib) => Ok((Gssapi::Heimdal, builder_from_pkgconfig(lib))),
            Err(lib) => Err(lib),
        },
    }
}

fn which() -> Gssapi {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap();

    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=GSS");
        return Gssapi::Apple;
    } else if target_os == "windows" {
        panic!("use SSPI on windows")
    } else if target_family == "unix" {
        let ldpath = env::var("LD_LIBRARY_PATH").unwrap_or(String::new());
        let paths = vec!["/lib", "/lib64", "/usr/lib", "/usr/lib64"];
        let krb5_path = Command::new("krb5-config")
            .arg("--prefix")
            .arg("gssapi")
            .output()
            .map(|o| o.stdout)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        let krb5_path = krb5_path.as_ref().map(|s| s.trim());
        for path in krb5_path.into_iter().chain(ldpath.split(':')).chain(paths) {
            if !path.is_empty() {
                if search_pat(path, "libgssapi_krb5.so*") {
                    println!("cargo:rustc-link-lib=gssapi_krb5");
                    return Gssapi::Mit;
                }
                if search_pat(path, "libgssapi.so*") {
                    println!("cargo:rustc-link-lib=gssapi");
                    return Gssapi::Heimdal;
                }
            }
        }
        panic!("no gssapi implementation found, install mit kerberos or heimdal");
    } else {
        panic!("libgssapi isn't ported to this platform yet")
    }
}

fn main() {
    let cross_compile = env::var("HOST").unwrap() != env::var("TARGET").unwrap();

    let (imp, builder) = match (cross_compile, try_pkgconfig()) {
        (false, Ok((imp, builder))) => (imp, builder),
        _ => {
            let imp = which();
            let builder = bindgen::Builder::default();
            let nix_cflags = env::var("NIX_CFLAGS_COMPILE");
            let builder = match imp {
                Gssapi::Mit | Gssapi::Heimdal => match nix_cflags {
                    Err(_) => builder,
                    Ok(flags) => builder.clang_args(flags.split(" ")),
                },
                Gssapi::Apple =>
                builder.clang_arg("-F/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/System/Library/Frameworks")
            };
            (imp, builder)
        }
    };
    let bindings = builder
        .allowlist_type("(OM_.+|gss_.+)")
        .allowlist_var("_?GSS_.+|gss_.+")
        .allowlist_function("gss_.*")
        .header(match imp {
            Gssapi::Mit => "src/wrapper_mit.h",
            Gssapi::Heimdal => "src/wrapper_heimdal.h",
            Gssapi::Apple => "src/wrapper_apple.h",
        })
        .generate()
        .expect("failed to generate gssapi bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write bindings")
}
