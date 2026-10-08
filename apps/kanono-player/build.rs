fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/icons/kanono-player.ico");
        res.set("ProductName", "Kanono Media Player");
        res.set("FileDescription", "Kanono Media Player");
        res.set("LegalCopyright", "Copyright (C) 2026 Kanono Contributors");
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resource: {e}");
        }
    }
}
