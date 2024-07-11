use std::path::PathBuf;

xflags::xflags! {
    cmd main {
        optional -f, --frequency frequency: u32
        optional -b, --benchmark
        optional -c, --count count: u32
        required path: PathBuf
    }
}
