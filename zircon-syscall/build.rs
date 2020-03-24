use std::io::Write;

fn main() {
    println!("cargo:rerun-if-changed=src/zx-syscall-numbers.h");

    let mut fout = std::fs::File::create("src/consts.rs").unwrap();
    writeln!(fout, "// Generated by build.rs. DO NOT EDIT.").unwrap();
    writeln!(fout, "use numeric_enum_macro::numeric_enum;\n").unwrap();
    writeln!(fout, "numeric_enum! {{").unwrap();
    writeln!(fout, "#[repr(u32)]").unwrap();
    writeln!(fout, "#[derive(Debug, Eq, PartialEq)]").unwrap();
    writeln!(fout, "#[allow(non_camel_case_types)]").unwrap();
    writeln!(fout, "pub enum SyscallType {{").unwrap();

    let data = std::fs::read_to_string("src/zx-syscall-numbers.h").unwrap();
    for line in data.split('\n') {
        if !line.starts_with("#define") {
            continue;
        }
        let mut iter = line.split(' ');
        let _ = iter.next().unwrap();
        let name = iter.next().unwrap();
        let id = iter.next().unwrap();

        let name = &name[7..].to_uppercase();
        writeln!(fout, "    {} = {},", name, id).unwrap();
    }
    writeln!(fout, "}}").unwrap();
    writeln!(fout, "}}").unwrap();
}
