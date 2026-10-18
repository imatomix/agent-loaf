use std::fs;
use std::path::Path;

use agent_loaf_core::base::messages::{table, Lang};

fn keys_in(text: &str, open: &str, close: char) -> Vec<String> {
    text.match_indices(open)
        // createElement("li") や split("/") も「t("」で終わるので、名前の途中に現れたものは除く
        .filter(|(i, _)| !text[..*i].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '.'))
        .filter_map(|(i, _)| {
            let rest = &text[i + open.len()..];
            rest.find(close).map(|end| rest[..end].to_string())
        })
        .collect()
}

#[test]
fn every_key_used_by_the_page_exists() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut used = Vec::new();
    let html = fs::read_to_string(app.join("index.html")).unwrap();
    used.extend(keys_in(&html, "data-i18n=\"", '"'));
    used.extend(keys_in(&html, "data-i18n-title=\"", '"'));
    for entry in fs::read_dir(app.join("src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ts") {
            used.extend(keys_in(&fs::read_to_string(&path).unwrap(), "t(\"", '"'));
        }
    }
    assert!(used.len() > 10, "キーを拾えていない: {used:?}");
    for key in used {
        assert!(table(Lang::Ja).contains_key(&key), "{key} が対応表にない");
    }
}
