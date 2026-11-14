use std::{fs, path::PathBuf};

const PLACEHOLDER: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>ytermusicd</title>
  </head>
  <body>
    <main id="app">
      <h1>ytermusicd</h1>
      <p>The web UI is not built. Run <code>npm install &amp;&amp; npm run build</code> in <code>crates/ytermusicd-web</code>.</p>
    </main>
  </body>
</html>
"#;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let dist = manifest.join("../ytermusicd-web/dist");
    println!("cargo:rerun-if-changed={}", dist.display());

    if dist.join("index.html").exists() {
        return;
    }

    fs::create_dir_all(&dist).expect("create web dist directory");
    fs::write(dist.join("index.html"), PLACEHOLDER).expect("write web placeholder");
}
