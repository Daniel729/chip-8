use std::path::PathBuf;

xflags::xflags! {
    cmd main {
        optional -f, --frequency frequency: u32
        required path: PathBuf
    }
}
