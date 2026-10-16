use agent_loaf_core::state::Status;
use tauri::image::Image;

// Figma の MenuBarIcon（18pt 四方）を scripts/make-tray-icons.sh で 36px にしたもの。
// macOS のメニューバーは画像の高さを 18pt に合わせて縮めるので、Retina で 1:1 になる。
// テンプレート画像として macOS が透明度だけを見て白か黒で塗るので、色は何でもよい
const DEFAULT: &[u8] = include_bytes!("../icons/tray/default.png");
const ERROR: &[u8] = include_bytes!("../icons/tray/error.png");
const RUNNING: &[u8] = include_bytes!("../icons/tray/play.png");
const NEEDS_ACTION: &[u8] = include_bytes!("../icons/tray/required.png");

pub fn tray_icon(status: Option<Status>) -> Image<'static> {
    let png = match status {
        Some(Status::NeedsAction) => NEEDS_ACTION,
        Some(Status::Error) => ERROR,
        Some(Status::Running) => RUNNING,
        Some(Status::Done | Status::Ended | Status::Unknown) | None => DEFAULT,
    };
    Image::from_bytes(png).expect("embedded menu bar icon is unreadable")
}

// ツールを使うたびに一覧が変わるが、アイコンが変わるのは状態の種類が変わったときだけ。毎回 PNG を読み直して設定し直さない
#[derive(Default)]
pub struct IconCache {
    shown: Option<Option<Status>>,
}

impl IconCache {
    pub fn needs_update(&mut self, status: Option<Status>) -> bool {
        if self.shown == Some(status) {
            return false;
        }
        self.shown = Some(status);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Option<Status>; 7] = [
        None,
        Some(Status::NeedsAction),
        Some(Status::Error),
        Some(Status::Running),
        Some(Status::Done),
        Some(Status::Ended),
        Some(Status::Unknown),
    ];

    #[test]
    fn every_icon_is_36px_square() {
        // 記号の有無で横幅が変わると、左隣のアイコンがずれる
        for status in ALL {
            let icon = tray_icon(status);
            assert_eq!((icon.width(), icon.height()), (36, 36), "{status:?}");
        }
    }

    #[test]
    fn shapes_are_fully_opaque() {
        // 半透明のまま渡すと、OS が薄く描くのに重なってほかのアイコンより薄く見える
        for status in ALL {
            let max_alpha = tray_icon(status).rgba().chunks(4).map(|px| px[3]).max();
            assert_eq!(max_alpha, Some(255), "{status:?}");
        }
    }

    #[test]
    fn only_statuses_with_a_symbol_change_the_icon() {
        let rgba = |status| tray_icon(status).rgba().to_vec();
        let idle = rgba(None);
        for status in [Status::Done, Status::Ended, Status::Unknown] {
            assert_eq!(rgba(Some(status)), idle, "{status:?}");
        }
        let symbols = [Status::NeedsAction, Status::Error, Status::Running].map(|s| rgba(Some(s)));
        assert!(symbols.iter().all(|icon| *icon != idle));
        assert_ne!(symbols[0], symbols[1]);
        assert_ne!(symbols[0], symbols[2]);
        assert_ne!(symbols[1], symbols[2]);
    }

    #[test]
    fn icon_is_redrawn_only_when_status_changes() {
        let mut cache = IconCache::default();
        assert!(cache.needs_update(None), "最初の 1 回は描く");
        assert!(!cache.needs_update(None));
        assert!(cache.needs_update(Some(Status::Running)));
        assert!(!cache.needs_update(Some(Status::Running)));
        assert!(cache.needs_update(Some(Status::NeedsAction)));
    }
}
