extern crate winres;

fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        // Only set icon if it exists to avoid build errors
        if std::path::Path::new("resources/app.ico").exists() {
            res.set_icon("resources/app.ico");
        }
        res.compile().unwrap();
    }
}
