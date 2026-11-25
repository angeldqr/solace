// Product name and version in the Windows file properties: code signing requires every signed
// binary to carry them, and they are what users see in Explorer and Task Manager.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "Solace")
            .set("FileDescription", "Solace command-line interface")
            .set("CompanyName", "Solace")
            .set("LegalCopyright", "Apache-2.0");
        res.compile().expect("failed to embed the Windows version resource");
    }
}
