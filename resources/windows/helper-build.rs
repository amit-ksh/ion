fn main() {
    #[cfg(windows)]
    {
        use std::{env, fs, path::PathBuf};
        println!("cargo:rerun-if-changed=../../resources/windows/helper-build.rs");
        println!("cargo:rerun-if-changed=../../resources/windows/AppIcon.ico");
        let package = env::var("CARGO_PKG_NAME").unwrap();
        let description = match package.as_str() {
            "waku-computer-use" => "ion Computer Use",
            "waku-daemon" => "ion daemon",
            _ => panic!("Unexpected helper package"),
        };
        let icon = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../resources/windows/AppIcon.ico")
            .to_string_lossy()
            .replace('\\', "\\\\");
        let version = env::var("CARGO_PKG_VERSION").unwrap();
        let numeric = format!("{},0", version.replace('.', ","));
        let resource = format!(
            r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEFLAGSMASK 0x3fL
FILEOS 0x40004L
FILETYPE 0x1L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "ion\0"
      VALUE "FileDescription", "{description}\0"
      VALUE "ProductName", "ion\0"
      VALUE "FileVersion", "{version}\0"
      VALUE "ProductVersion", "{version}\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0409, 1200
  END
END
"#
        );
        let file = PathBuf::from(env::var("OUT_DIR").unwrap()).join("ion-helper.rc");
        fs::write(&file, resource).unwrap();
        embed_resource::compile(&file, embed_resource::NONE)
            .manifest_optional()
            .expect("embed ion helper identity");
    }
}
