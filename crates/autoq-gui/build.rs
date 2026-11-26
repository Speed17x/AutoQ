fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        let ico = std::path::Path::new("../../windows-beta/autoq.ico");
        if ico.is_file() {
            if let Some(p) = ico.to_str() {
                res.set_icon(p);
            }
        }
        res.set("ProductName", "AutoQ");
        res.set("FileDescription", "AutoQ — Universal Parametric EQ & AutoEQ");
        res.set("CompanyName", "AutoQ (Open Source)");
        res.set("LegalCopyright", "GPL-2.0-or-later");
        if let Err(e) = res.compile() {
            println!("cargo:warning=winresource compile skipped: {e}");
        }
    }
}
