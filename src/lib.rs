use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Version {
    Lua51,
    Lua52,
    Lua53,
    Lua54,
}
pub use self::Version::*;

pub struct Build {
    out_dir: Option<PathBuf>,
    target: Option<String>,
    host: Option<String>,
}

pub struct Artifacts {
    include_dir: PathBuf,
    lib_dir: PathBuf,
    libs: Vec<String>,
}

impl Build {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Build {
        Build {
            out_dir: env::var_os("OUT_DIR").map(|s| PathBuf::from(s).join("lua-build")),
            target: env::var("TARGET").ok(),
            host: env::var("HOST").ok(),
        }
    }

    pub fn out_dir<P: AsRef<Path>>(&mut self, path: P) -> &mut Build {
        self.out_dir = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn target(&mut self, target: &str) -> &mut Build {
        self.target = Some(target.to_string());
        self
    }

    pub fn host(&mut self, host: &str) -> &mut Build {
        self.host = Some(host.to_string());
        self
    }

    pub fn build(&mut self, version: Version) -> Artifacts {
        let target = &self.target.as_ref().expect("TARGET not set")[..];
        let host = &self.host.as_ref().expect("HOST not set")[..];
        let out_dir = self.out_dir.as_ref().expect("OUT_DIR not set");
        let lib_dir = out_dir.join("lib");
        let include_dir = out_dir.join("include");

        let source_dir_base = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut source_dir = match version {
            Lua51 => source_dir_base.join("lua-5.1.5"),
            Lua52 => source_dir_base.join("lua-5.2.4"),
            Lua53 => source_dir_base.join("lua-5.3.6"),
            Lua54 => source_dir_base.join("lua-5.4.7"),
        };

        if lib_dir.exists() {
            fs::remove_dir_all(&lib_dir).unwrap();
        }
        fs::create_dir_all(&lib_dir).unwrap();

        if include_dir.exists() {
            fs::remove_dir_all(&include_dir).unwrap();
        }
        fs::create_dir_all(&include_dir).unwrap();

        let mut config = cc::Build::new();
        config
            .target(target)
            .host(host)
            .warnings(false)
            .opt_level(2)
            .cargo_metadata(false);

        match target {
            _ if target.contains("linux") => {
                config.define("LUA_USE_LINUX", None);
            }
            _ if target.ends_with("bsd") => {
                config.define("LUA_USE_LINUX", None);
            }
            _ if target.contains("apple-darwin") => {
                match version {
                    Lua51 => config.define("LUA_USE_LINUX", None),
                    _ => config.define("LUA_USE_MACOSX", None),
                };
            }
            _ if target.contains("apple-ios") => {
                match version {
                    Lua54 => config.define("LUA_USE_IOS", None),
                    _ => config.define("LUA_USE_POSIX", None),
                };
            }
            _ if target.contains("windows") => {
                // Defined in Lua >= 5.3
                config.define("LUA_USE_WINDOWS", None);
            }
            _ if target.ends_with("emscripten") => {
                config
                    .define("LUA_USE_POSIX", None)
                    .cpp(true)
                    .flag("-fexceptions"); // Enable exceptions to be caught

                let cpp_source_dir = out_dir.join("cpp_source");
                if cpp_source_dir.exists() {
                    fs::remove_dir_all(&cpp_source_dir).unwrap();
                }
                fs::create_dir_all(&cpp_source_dir).unwrap();

                for file in fs::read_dir(&source_dir).unwrap() {
                    let file = file.unwrap();
                    let filename = file.file_name();
                    let filename = filename.to_str().unwrap();
                    let src_file = source_dir.join(file.file_name());
                    let dst_file = cpp_source_dir.join(file.file_name());

                    let mut content = fs::read(src_file).unwrap();
                    if ["lauxlib.h", "lua.h", "lualib.h"].contains(&filename) {
                        content.splice(0..0, b"extern \"C\" {\n".to_vec());
                        content.extend(b"\n}".to_vec())
                    }
                    fs::write(dst_file, content).unwrap();
                }
                source_dir = cpp_source_dir
            }
            _ => panic!("don't know how to build Lua for {}", target),
        };

        if let Lua54 = version {
            config.define("LUA_COMPAT_5_3", None);
            #[cfg(feature = "ucid")]
            config.define("LUA_UCID", None);
        }

        if cfg!(debug_assertions) {
            config.define("LUA_USE_APICHECK", None);
        }

        if env::var("CARGO_FEATURE_THREAD").is_ok() {
            config.define("LUA_USER_H", "\"../ezlua/ezlua.h\"");
        }

        if env::var("CARGO_FEATURE_BUILD_DYLIB").is_ok() {
            let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap();
            if target_family == "windows" {
                config.define("LUA_CORE", None);
                config.define("LUA_BUILD_AS_DLL", None);
            } else {
                config.pic(true);
                config.flag("-fvisibility=default");
                config.shared_flag(true);
                println!("cargo:rustc-cdylib-link-arg=-fvisibility=default");
            }
        }

        let lib_name = match version {
            Lua51 => "lua5.1",
            Lua52 => "lua5.2",
            Lua53 => "lua5.3",
            Lua54 => "lua5.4",
        };

        config
            .include(&source_dir)
            .flag("-w") // Suppress all warnings
            .flag_if_supported("-fno-common") // Compile common globals like normal definitions
            .file(source_dir.join("lapi.c"))
            .file(source_dir.join("lauxlib.c"))
            .file(source_dir.join("lbaselib.c"))
            // skipped: lbitlib.c (>= 5.2, <= 5.3)
            .file(source_dir.join("lcode.c"))
            // skipped: lcorolib.c (>= 5.2)
            // skipped: lctype.c (>= 5.2)
            .file(source_dir.join("ldblib.c"))
            .file(source_dir.join("ldebug.c"))
            .file(source_dir.join("ldo.c"))
            .file(source_dir.join("ldump.c"))
            .file(source_dir.join("lfunc.c"))
            .file(source_dir.join("lgc.c"))
            .file(source_dir.join("linit.c"))
            .file(source_dir.join("liolib.c"))
            .file(source_dir.join("llex.c"))
            .file(source_dir.join("lmathlib.c"))
            .file(source_dir.join("lmem.c"))
            .file(source_dir.join("loadlib.c"))
            .file(source_dir.join("lobject.c"))
            .file(source_dir.join("lopcodes.c"))
            .file(source_dir.join("loslib.c"))
            .file(source_dir.join("lparser.c"))
            .file(source_dir.join("lstate.c"))
            .file(source_dir.join("lstring.c"))
            .file(source_dir.join("lstrlib.c"))
            .file(source_dir.join("ltable.c"))
            .file(source_dir.join("ltablib.c"))
            .file(source_dir.join("ltm.c"))
            .file(source_dir.join("lundump.c"))
            // skipped: lutf8lib.c (>= 5.3)
            .file(source_dir.join("lvm.c"))
            .file(source_dir.join("lzio.c"));

        match version {
            Lua51 => {}
            Lua52 => {
                config
                    .file(source_dir.join("lbitlib.c"))
                    .file(source_dir.join("lcorolib.c"))
                    .file(source_dir.join("lctype.c"));
            }
            Lua53 => {
                config
                    .file(source_dir.join("lbitlib.c"))
                    .file(source_dir.join("lcorolib.c"))
                    .file(source_dir.join("lctype.c"))
                    .file(source_dir.join("lutf8lib.c"));
            }
            Lua54 => {
                config
                    .file(source_dir.join("lcorolib.c"))
                    .file(source_dir.join("lctype.c"))
                    .file(source_dir.join("lutf8lib.c"));
            }
        }

        config.out_dir(&lib_dir).compile(lib_name);

        for f in &["lauxlib.h", "lua.h", "luaconf.h", "lualib.h"] {
            fs::copy(source_dir.join(f), include_dir.join(f)).unwrap();
        }

        Artifacts {
            lib_dir,
            include_dir,
            libs: vec![lib_name.to_string()],
        }
    }
}

impl Artifacts {
    pub fn include_dir(&self) -> &Path {
        &self.include_dir
    }

    pub fn lib_dir(&self) -> &Path {
        &self.lib_dir
    }

    pub fn libs(&self) -> &[String] {
        &self.libs
    }

    pub fn print_cargo_metadata(&self) {
        println!("cargo:rustc-link-search=native={}", self.lib_dir.display());
        for lib in self.libs.iter() {
            println!("cargo:rustc-link-lib=static={}", lib);
        }
        println!("cargo:include={}", self.include_dir.display());
        println!("cargo:lib={}", self.lib_dir.display());
    }
}
