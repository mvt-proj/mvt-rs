use std::fs;

fn main() {
    let dirs = ["map_assets/sprites", "map_assets/glyphs"];

    for dir in dirs.iter() {
        if !fs::metadata(dir).is_ok() {
            fs::create_dir_all(dir).expect(&format!("The {} directory could not be created", dir));
        }
    }
}
