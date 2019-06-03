
use std::io::{Write, Result};
use std::fs::File;
use std::path::{PathBuf};
use std::process::Command;
use std::collections::HashSet;
use clap::{Arg, App};
use tempfile::tempdir;
use log::info;
use ar;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref BLACKLISTED_OBJS: HashSet<String> = {
        let res: HashSet<_> = [
            "__tls_get_addr.lo"
        ].iter().map(|x| String::from(*x)).collect();
        res
    };
}

fn main() -> Result<()> {
    let matches = App::new("rust staticlib linker")
        .version("1.0")
        .author("Baojun Wang <wangbj@gmail.com>")
        .about("generate freestanding shared libraries from rust staticlib")
        .arg(Arg::with_name("staticlib")
             .long("staticlib")
             .value_name("STATICLIB")
             .help("staticlib produced by rust create-type=`static-lib`")
             .required(true)
             .takes_value(true))
        .arg(Arg::with_name("staticcrt")
             .help("static crt lib to link with, such as libc.a")
             .long("staticcrt")
             .takes_value(true)
             .required(true))
        .arg(Arg::with_name("export")
             .long("export")
             .multiple(true)
             .takes_value(true)
             .required(true)
             .help("symbol name to export"))
        .arg(Arg::with_name("output")
             .long("output")
             .short("o")
             .takes_value(true)
             .required(true)
             .help("output file name"))
        .arg(Arg::with_name("soname")
             .long("soname")
             .takes_value(true)
             .help("pass optional soname to the linker"))
        .arg(Arg::with_name("with-ld")
             .long("with-ld")
             .takes_value(true)
             .help("use supplied GNU ld"))
        .get_matches();

    env_logger::init();

    let ld = matches.value_of("with-ld").unwrap_or("ld");
    let crt = matches.value_of("staticcrt").unwrap();
    let staticlib = PathBuf::from(matches.value_of("staticlib").unwrap()).canonicalize()?;
    let staticlib_shortname = PathBuf::from(&staticlib).canonicalize()?;
    let output = matches.value_of("output").unwrap();
    let soname = matches.value_of("soname").map(|s| s.to_string()).or_else(|| {
        staticlib_shortname.file_name().and_then(|f_os| {
            f_os.to_str().and_then(|f| {
                let s = String::from(f);
                if s.ends_with(".a") {
                    let mut t = String::from(&s[0..s.len()-2]);
                    t += ".so";
                    Some(t)
                } else {
                    Some(s)
                }
            })
        })
    }).unwrap();
    let soname_short = {
        let mut start = 0;
        let mut end = soname.len();
        if soname.starts_with("lib") {
            start = 3;
        }
        if soname.ends_with(".so") {
            end -= 3;
        }
        String::from(&soname[start..end])
    };

    let exports = matches.values_of("export").map(|vs| vs.collect::<Vec<_>>()).unwrap_or_else(||Vec::new());

    let mut linker_script = String::from("VERSION {\n");
    linker_script.push_str(&format!("  {} {{\n", soname_short));
    linker_script.push_str("    global:\n");
    exports.iter().for_each(|e| {
        linker_script.push_str(&format!("      {};\n", e));
    });
    linker_script.push_str("    local:\n");
    linker_script.push_str("      *;\n");
    linker_script.push_str("  };\n");
    linker_script.push_str("}\n");

    let dir = tempdir()?;
    let mut archive = ar::Archive::new(File::open(staticlib.to_owned()).unwrap());
    let mut objs: HashSet<String> = HashSet::new();
    let mut cmd = Command::new(ld);
    while let Some(entry_result) = archive.next_entry() {
        let mut entry = entry_result?;
        let fname = std::str::from_utf8(entry.header().identifier()).unwrap().to_owned();
        if !BLACKLISTED_OBJS.contains(&fname) {
            let fqdn = dir.path().join(&fname);
            let mut file = File::create(fqdn.to_owned())?;
            std::io::copy(&mut entry, &mut file)?;
            objs.insert(fqdn.to_str().unwrap().to_string());
        }
    }

    let mut ldsfile = tempfile::NamedTempFile::new()?;
    ldsfile.write(linker_script.as_bytes())?;
    let ldscript = ldsfile.path().canonicalize()?.to_str().unwrap().to_owned();
    let objs: Vec<String> = objs.iter().cloned().collect();
    cmd.args(objs.as_slice());
    cmd.arg(crt);
    cmd.args(&["-o", output]);
    cmd.arg("-shared");
    cmd.arg("-fPIC");
    cmd.arg("-flto");
    cmd.arg("-no-undefined");
    cmd.arg("-nostdlib");
    cmd.args(&["-T", ldscript.as_ref()]);

    info!("cmdline: {:#?}", cmd);

    let exit_code = cmd.status()?;
    std::process::exit(exit_code.code().unwrap_or(0));
}
