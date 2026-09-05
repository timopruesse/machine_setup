fn main() {
    // Embed manifest on Windows to prevent "setup" in the name
    // from triggering the installer detection heuristic.
    // `winresource` is a windows-host build-dep; gate compile-time use the same way.
    #[cfg(windows)]
    {
        // Prefer CARGO_CFG_TARGET_OS so a Windows host targeting non-Windows
        // does not embed a Windows resource by mistake.
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_manifest(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#,
            );
            if let Err(e) = res.compile() {
                panic!("failed to compile Windows resources: {e}");
            }
        }
    }
}
