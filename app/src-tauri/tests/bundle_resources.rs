use std::fs;
use std::path::{Path, PathBuf};

fn files_under(root: &Path, relative: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root.join(relative)).unwrap() {
        let entry = entry.unwrap();
        let path = relative.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            files_under(root, &path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn every_plugin_file_is_bundled_into_marketplace() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest_dir.join("../..");
    let conf: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(manifest_dir.join("tauri.conf.json")).unwrap()).unwrap();
    let resources = conf["bundle"]["resources"]
        .as_object()
        .expect("bundle.resources がない");
    let mut files = vec![PathBuf::from(".claude-plugin/marketplace.json")];
    files_under(&repo, Path::new("plugin"), &mut files);
    for file in files {
        let source = format!("../../{}", file.display());
        let dest = format!("marketplace/{}", file.display());
        assert_eq!(
            resources.get(&source).and_then(|v| v.as_str()),
            Some(dest.as_str()),
            "{source} を bundle.resources に足すこと"
        );
    }
}
