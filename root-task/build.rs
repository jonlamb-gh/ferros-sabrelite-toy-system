use ferros_build::*;
use std::path::Path;

fn main() {
    let out_dir = Path::new(&std::env::var_os("OUT_DIR").unwrap()).to_owned();
    let bin_dir = out_dir.join("..").join("..").join("..");
    let resources = out_dir.join("resources.rs");

    let iomux = ElfResource {
        path: bin_dir.join("iomux"),
        image_name: "iomux".to_owned(),
        type_name: "Iomux".to_owned(),
        stack_size_bits: Some(14),
    };
    println!("cargo:rerun-if-changed={}", iomux.path.display());

    let persistent_storage = ElfResource {
        path: bin_dir.join("persistent-storage"),
        image_name: "persistent-storage".to_owned(),
        type_name: "PersistentStorage".to_owned(),
        stack_size_bits: Some(14),
    };
    println!(
        "cargo:rerun-if-changed={}",
        persistent_storage.path.display()
    );

    let console = ElfResource {
        path: bin_dir.join("console"),
        image_name: "console".to_owned(),
        type_name: "Console".to_owned(),
        stack_size_bits: Some(14),
    };
    println!("cargo:rerun-if-changed={}", console.path.display());

    let procs = vec![
        &iomux as &dyn Resource,
        &persistent_storage as &dyn Resource,
        &console as &dyn Resource,
    ];

    embed_resources(&resources, procs);
}
