use agent_loaf_core::base::messages::Lang;

pub fn detect() -> Lang {
    let tag = preferred_localization().or_else(|| std::env::var("LANG").ok());
    let lang = tag.as_deref().map(Lang::from_tag).unwrap_or(Lang::En);
    eprintln!("language: {lang:?} (from {tag:?})");
    lang
}

// macOS の優先言語と「アプリごとの言語」の両方を、アプリが宣言した言語（Info.plist）の中から選んだ結果
#[cfg(target_os = "macos")]
fn preferred_localization() -> Option<String> {
    use objc2_foundation::NSBundle;
    let bundle = NSBundle::mainBundle();
    bundle.bundleIdentifier()?;
    bundle.preferredLocalizations().firstObject().map(|tag| tag.to_string())
}

#[cfg(not(target_os = "macos"))]
fn preferred_localization() -> Option<String> {
    None
}
