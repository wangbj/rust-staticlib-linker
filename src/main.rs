
use std::io::{Write, Result};
use std::fs::File;
use std::path::{PathBuf};
use std::process::Command;
use clap::{Arg, App};
use tempfile::tempdir;
use log::{info, debug};
use ar;

// linker script prelude
fn generate_lds_prelude() -> String {
    let output = vec![ "OUTPUT_FORMAT(elf64-x86-64)",
                        "OUTPUT_ARCH(i386:x86-64)",
                        ""];
    output.join("\n")
}

// generate VERSION section for GNU linker script
fn generate_lds_version(soname: &str, symbols: Vec<String>) -> String {
    let mut linker_script = String::from("VERSION {\n");
    linker_script.push_str(&format!("  {} {{\n", soname));
    linker_script.push_str("    global:\n");
    symbols.iter().for_each(|e| {
        linker_script.push_str(&format!("      {};\n", e));
    });
    linker_script.push_str("    local:\n");
    linker_script.push_str("      *;\n");
    linker_script.push_str("  };\n");
    linker_script.push_str("}\n");

    linker_script
}

// linker script SECTIONS
fn generate_lds_section() -> String {
    let sections = vec![
        "SECTIONS",
        "{",
        "  /* Read-only sections, merged into text segment: */",
        "  .interp         : { *(.interp) }",
        "  .hash           : { *(.hash) }",
        "  .gnu.hash       : { *(.gnu.hash) }",
        "  .dynsym         : { *(.dynsym) }",
        "  .dynstr         : { *(.dynstr) }",
        "  .gnu.version    : { *(.gnu.version) }",
        "  .rela.data.rel.ro   : { *(.rela.data.rel.ro .rela.data.rel.ro.* .rela.gnu.linkonce.d.rel.ro.*) }",
        "  .rela.data      : { *(.rela.data .rela.data.* .rela.gnu.linkonce.d.*) }",
        "  .rela.tdata	  : { *(.rela.tdata .rela.tdata.* .rela.gnu.linkonce.td.*) }",
        "  .rela.tbss	  : { *(.rela.tbss .rela.tbss.* .rela.gnu.linkonce.tb.*) }",
        "  .rela.got       : { *(.rela.got) }",
        "  .rela.bss       : { *(.rela.bss .rela.bss.* .rela.gnu.linkonce.b.*) }",
        "  .rela.ldata     : { *(.rela.ldata .rela.ldata.* .rela.gnu.linkonce.l.*) }",
        "  .rela.lbss      : { *(.rela.lbss .rela.lbss.* .rela.gnu.linkonce.lb.*) }",
        "  .rela.lrodata   : { *(.rela.lrodata .rela.lrodata.* .rela.gnu.linkonce.lr.*) }",
        "  .rela.ifunc     : { *(.rela.ifunc) }",
        "  .rela.plt       :",
        "    {",
        "      *(.rela.plt)",
        "    }",
        "  .plt            : { *(.plt) }",
        "  .plt.got        : { *(.plt.got) }",
        "  .text           :",
        "  {",
        "    *(.text)",
        "    PROVIDE_HIDDEN (__tls_get_new = __tls_get_addr);",
        "    PROVIDE_HIDDEN (__tls_get_addr_start = .);",
        "    PROVIDE(__tls_get_addr = .);",
        "    *(.text.__tls_get_addr)",
        "    . += 0x80;",
        "    PROVIDE_HIDDEN (__tls_get_addr_end = .);",
        "    *(.text.*)",
        "  }",
        "  PROVIDE (__etext = .);",
        "  PROVIDE (_etext = .);",
        "  PROVIDE (etext = .);",
        "  .rodata         : { *(.rodata .rodata.* ) }",
        "  .eh_frame       : ONLY_IF_RO { KEEP (*(.eh_frame)) *(.eh_frame.*) }",
        "  .gcc_except_table   : ONLY_IF_RO { *(.gcc_except_table .gcc_except_table.*) }",
        "  /* Adjust the address for the data segment.  We want to adjust up to",
        "     the same address within the page on the next page up.  */",
        "  . = DATA_SEGMENT_ALIGN (CONSTANT (MAXPAGESIZE), CONSTANT (COMMONPAGESIZE));",
        "  /* Thread Local Storage sections  */",
        "  .tdata	  : { *(.tdata .tdata.*) }",
        "  .tbss		  : { *(.tbss .tbss.*) }",
        "  .preinit_array     :",
        "  {",
        "    PROVIDE_HIDDEN (__preinit_array_start = .);",
        "    KEEP (*(.preinit_array))",
        "    PROVIDE_HIDDEN (__preinit_array_end = .);",
        "  }",
        "  .init_array     :",
        "  {",
        "    PROVIDE_HIDDEN (__init_array_start = .);",
        "    KEEP (*(SORT_BY_INIT_PRIORITY(.init_array.*) SORT_BY_INIT_PRIORITY(.ctors.*)))",
        "    KEEP (*(.init_array .ctors))",
        "    PROVIDE_HIDDEN (__init_array_end = .);",
        "  }",
        "  .fini_array     :",
        "  {",
        "    PROVIDE_HIDDEN (__fini_array_start = .);",
        "    KEEP (*(SORT_BY_INIT_PRIORITY(.fini_array.*) SORT_BY_INIT_PRIORITY(.dtors.*)))",
        "    KEEP (*(.fini_array .dtors))",
        "    PROVIDE_HIDDEN (__fini_array_end = .);",
        "  }",
        "  .data.rel.ro : { *(.data.rel.ro.local* ) *(.data.rel.ro .data.rel.ro.* ) }",
        "  .dynamic        : { *(.dynamic) }",
        "  .got            : { *(.got) }",
        "  . = DATA_SEGMENT_RELRO_END (SIZEOF (.got.plt) >= 24 ? 24 : 0, .);",
        "  .got.plt        : { *(.got.plt)  *(.igot.plt) }",
        "  .data           :",
        "  {",
        "    *(.data .data.* )",
        "    SORT(CONSTRUCTORS)",
        "  }",
        "  _edata = .; PROVIDE (edata = .);",
        "  __bss_start = .;",
        "  .bss            :",
        "  {",
        "   *(.dynbss)",
        "   *(.bss .bss.* )",
        "   *(COMMON)",
        "   /* Align here to ensure that the .bss section occupies space up to",
        "      _end.  Align after .bss to ensure correct alignment even if the",
        "      .bss section disappears because there are no input sections.",
        "      FIXME: Why do we need it? When there is no .bss section, we don't",
        "      pad the .data section.  */",
        "   . = ALIGN(. != 0 ? 8 : 1);",
        "  }",
        "  . = ALIGN(8);",
        "  _end = .; PROVIDE (end = .);",
        "  . = DATA_SEGMENT_END (.);",
        "  /* Stabs debugging sections.  */",
        "  .stab          0 : { *(.stab) }",
        "  .stabstr       0 : { *(.stabstr) }",
        "  .stab.excl     0 : { *(.stab.excl) }",
        "  .stab.exclstr  0 : { *(.stab.exclstr) }",
        "  .stab.index    0 : { *(.stab.index) }",
        "  .stab.indexstr 0 : { *(.stab.indexstr) }",
        "  .comment       0 : { *(.comment) }",
        "  /* DWARF debug sections.",
        "     Symbols in the DWARF debugging sections are relative to the beginning",
        "     of the section so we begin them at 0.  */",
        "  /* DWARF 1 */",
        "  .debug          0 : { *(.debug) }",
        "  .line           0 : { *(.line) }",
        "  /* GNU DWARF 1 extensions */",
        "  .debug_srcinfo  0 : { *(.debug_srcinfo) }",
        "  .debug_sfnames  0 : { *(.debug_sfnames) }",
        "  /* DWARF 1.1 and DWARF 2 */",
        "  .debug_aranges  0 : { *(.debug_aranges) }",
        "  .debug_pubnames 0 : { *(.debug_pubnames) }",
        "  /* DWARF 2 */",
        "  .debug_info     0 : { *(.debug_info .gnu.linkonce.wi.*) }",
        "  .debug_abbrev   0 : { *(.debug_abbrev) }",
        "  .debug_line     0 : { *(.debug_line .debug_line.* .debug_line_end ) }",
        "  .debug_frame    0 : { *(.debug_frame) }",
        "  .debug_str      0 : { *(.debug_str) }",
        "  .debug_loc      0 : { *(.debug_loc) }",
        "  .debug_macinfo  0 : { *(.debug_macinfo) }",
        "  /* DWARF 3 */",
        "  .debug_pubtypes 0 : { *(.debug_pubtypes) }",
        "  .debug_ranges   0 : { *(.debug_ranges) }",
        "  /* DWARF Extension.  */",
        "  .debug_macro    0 : { *(.debug_macro) }",
        "  .debug_addr     0 : { *(.debug_addr) }",
        "  .gnu.attributes 0 : { KEEP (*(.gnu.attributes)) }",
        "  /DISCARD/ : { *(.note.GNU-stack) *(.gnu_debuglink) *(.gnu.lto_*) }",
        "}",
        ""];
    sections.join("\n")
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

    let exports = matches.values_of("export").map(|vs| vs.map(|s|s.to_string()).collect::<Vec<_>>()).unwrap_or_else(||Vec::new());

    let dir = tempdir()?;
    let mut archive = ar::Archive::new(File::open(staticlib.to_owned()).unwrap());
    let mut objs: Vec<String> = Vec::new();
    let mut cmd = Command::new(ld);
    let mut k = 0;
    while let Some(entry_result) = archive.next_entry() {
        let mut entry = entry_result?;
        let fname = format!("{:<04}_", k) + std::str::from_utf8(entry.header().identifier()).unwrap();
        let fqdn = dir.path().join(&fname);
        let mut file = File::create(fqdn.to_owned())?;
        std::io::copy(&mut entry, &mut file)?;
        objs.push(fqdn.to_str().unwrap().to_string());
        k = 1 + k;
    }

    let mut ldsfile = tempfile::NamedTempFile::new()?;
    let linker_script = vec![generate_lds_prelude(),
                             generate_lds_version(&soname_short, exports)
                             , generate_lds_section()].join("\n");
    ldsfile.write(linker_script.as_bytes())?;
    let ldscript = ldsfile.path().canonicalize()?.to_str().unwrap().to_owned();
    let objs: Vec<String> = objs.iter().cloned().collect();
    cmd.args(objs.as_slice());
    cmd.args(&["-o", output]);
    cmd.arg("-Bstatic");
    cmd.arg("-shared");
    cmd.arg("-fPIC");
    cmd.arg("-flto");
    cmd.arg("-no-undefined");
    cmd.arg("-nostdlib");
    cmd.args(&["-T", ldscript.as_ref()]);

    info!("cmdline: {:#?}", cmd);
    debug!("{}", linker_script);

    let exit_code = cmd.status()?;
    std::process::exit(exit_code.code().unwrap_or(0));
}
