use crate::setup::plugin_status::DirState;

// None は「まだ調べ終わっていない」。起動直後にわからないことを問題として数えると、何もないのにバッジが出る
pub fn needs_attention(
    states: Option<&[DirState]>,
    claude_found: Option<bool>,
    notifications_allowed: Option<bool>,
) -> bool {
    let dir_problem = states.is_some_and(|states| {
        states.is_empty()
            || states
                .iter()
                .any(|s| matches!(s, DirState::NotInstalled | DirState::Missing | DirState::NeedsTrust))
    });
    notifications_allowed == Some(false) || claude_found == Some(false) || dir_problem
}

#[cfg(test)]
mod tests {
    use super::*;
    use DirState::*;

    #[test]
    fn badge_when_something_needs_the_user() {
        let ok = Some(true);
        assert!(!needs_attention(Some(&[Installed]), ok, ok));
        assert!(needs_attention(Some(&[Installed]), ok, Some(false)), "通知がオフ");
        assert!(
            needs_attention(Some(&[Installed]), Some(false), ok),
            "claude が見つからない"
        );
        assert!(
            needs_attention(Some(&[]), ok, ok),
            "入っているディレクトリが 1 つもない"
        );
        assert!(
            needs_attention(Some(&[Installed, NotInstalled]), ok, ok),
            "入っていないディレクトリがある"
        );
        assert!(needs_attention(Some(&[Installed, Missing]), ok, ok), "フォルダーがない");
        assert!(
            needs_attention(Some(&[Installed, NeedsTrust]), ok, ok),
            "Codex の hook の許可が要る"
        );
    }

    #[test]
    fn nothing_unknown_yet_counts_as_a_problem() {
        assert!(!needs_attention(None, None, None), "起動直後はまだ何もわかっていない");
        assert!(
            !needs_attention(Some(&[Checking]), Some(true), Some(true)),
            "確認中のディレクトリは問題に数えない"
        );
        assert!(
            !needs_attention(Some(&[Installed, Checking]), Some(true), None),
            "通知の許可の答えを待っている"
        );
        assert!(
            needs_attention(Some(&[Checking, NotInstalled]), None, None),
            "わかっている問題があればバッジ"
        );
    }
}
