use ferros_build::*;
use std::path::Path;

fn main() {
    let out_dir = Path::new(&std::env::var_os("OUT_DIR").unwrap()).to_owned();
    let bin_dir = out_dir.join("..").join("..").join("..");
    let resources = out_dir.join("resources.rs");

    let console = ElfResource {
        path: bin_dir.join("console"),
        image_name: "console".to_owned(),
        type_name: "Console".to_owned(),
        stack_size_bits: None,
    };

    embed_resources(&resources, vec![&console as &dyn Resource]);

    // Make sure root-task gets rebuilt if anything changes
    // since we stuff our elf procs in the binary
    println!("cargo:rerun-if-changed=../applications");
    println!("cargo:rerun-if-changed=../drivers");
}
