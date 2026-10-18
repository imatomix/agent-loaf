use serde::Serialize;

pub const FIRST_CHECK_DELAY_MS: u64 = 60_000;
pub const CHECK_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;
// 失敗した確認（オフラインなど）は、少し後にもう一度試す。失敗はすぐ終わるので、負荷はほぼない
pub const RETRY_AFTER_FAILURE_MS: u64 = 30 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateState {
    #[default]
    Idle,
    Available {
        version: String,
    },
    Downloading {
        version: String,
        percent: u8,
    },
    Failed {
        version: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateEvent {
    Found(String),
    NotFound,
    CheckFailed,
    Started,
    Progress { downloaded: u64, total: Option<u64> },
    InstallFailed,
}

impl UpdateState {
    fn version(&self) -> Option<&str> {
        match self {
            UpdateState::Idle => None,
            UpdateState::Available { version }
            | UpdateState::Downloading { version, .. }
            | UpdateState::Failed { version } => Some(version),
        }
    }

    pub fn next(&self, event: &UpdateEvent) -> UpdateState {
        let downloading = matches!(self, UpdateState::Downloading { .. });
        match event {
            // ダウンロードの途中に届いた確認の結果で、進み具合の表示を消さない
            _ if downloading && !matches!(event, UpdateEvent::Progress { .. } | UpdateEvent::InstallFailed) => {
                self.clone()
            }
            UpdateEvent::Found(version) => UpdateState::Available {
                version: version.clone(),
            },
            UpdateEvent::NotFound => UpdateState::Idle,
            UpdateEvent::CheckFailed => self.clone(),
            UpdateEvent::Started => match self.version() {
                Some(version) => UpdateState::Downloading {
                    version: version.to_string(),
                    percent: 0,
                },
                None => self.clone(),
            },
            UpdateEvent::Progress { downloaded, total } => match (self, total) {
                (UpdateState::Downloading { version, .. }, Some(total)) if *total > 0 => {
                    let percent = (downloaded.saturating_mul(100) / total).min(100) as u8;
                    UpdateState::Downloading {
                        version: version.clone(),
                        percent,
                    }
                }
                _ => self.clone(),
            },
            UpdateEvent::InstallFailed => match self.version() {
                Some(version) => UpdateState::Failed {
                    version: version.to_string(),
                },
                None => self.clone(),
            },
        }
    }
}

// 時刻は壁時計で比べる。スリープ中に止まる時計で数えると、何日も眠っていた Mac が戻っても確かめない
pub fn is_due(now_ms: u64, started_ms: u64, last_check_ms: Option<u64>, last_check_failed: bool) -> bool {
    let interval = if last_check_failed {
        RETRY_AFTER_FAILURE_MS
    } else {
        CHECK_INTERVAL_MS
    };
    match last_check_ms {
        None => now_ms >= started_ms + FIRST_CHECK_DELAY_MS,
        Some(last) => now_ms < last || now_ms - last >= interval,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available() -> UpdateState {
        UpdateState::Available {
            version: "0.5.1".into(),
        }
    }

    #[test]
    fn found_version_becomes_available_and_quiet_failures_change_nothing() {
        assert_eq!(UpdateState::Idle.next(&UpdateEvent::Found("0.5.1".into())), available());
        assert_eq!(UpdateState::Idle.next(&UpdateEvent::NotFound), UpdateState::Idle);
        // オフラインなどで確かめられなかったときは、何も表示しない
        assert_eq!(UpdateState::Idle.next(&UpdateEvent::CheckFailed), UpdateState::Idle);
        assert_eq!(
            available().next(&UpdateEvent::CheckFailed),
            available(),
            "前に見つけた版の知らせは消さない"
        );
    }

    #[test]
    fn download_reports_percent_and_failure_keeps_the_version() {
        let started = available().next(&UpdateEvent::Started);
        assert_eq!(
            started,
            UpdateState::Downloading {
                version: "0.5.1".into(),
                percent: 0
            }
        );
        let half = started.next(&UpdateEvent::Progress {
            downloaded: 50,
            total: Some(200),
        });
        assert_eq!(
            half,
            UpdateState::Downloading {
                version: "0.5.1".into(),
                percent: 25
            }
        );
        assert_eq!(
            half.next(&UpdateEvent::Progress {
                downloaded: 10,
                total: None
            }),
            half,
            "大きさが分からなければ % を変えない"
        );
        assert_eq!(
            half.next(&UpdateEvent::Progress {
                downloaded: 999,
                total: Some(200)
            }),
            UpdateState::Downloading {
                version: "0.5.1".into(),
                percent: 100
            }
        );
        let failed = half.next(&UpdateEvent::InstallFailed);
        assert_eq!(
            failed,
            UpdateState::Failed {
                version: "0.5.1".into()
            }
        );
        assert_eq!(
            failed.next(&UpdateEvent::Started),
            UpdateState::Downloading {
                version: "0.5.1".into(),
                percent: 0
            },
            "もう一度"
        );
    }

    #[test]
    fn downloading_ignores_new_checks() {
        let downloading = UpdateState::Downloading {
            version: "0.5.1".into(),
            percent: 40,
        };
        assert_eq!(downloading.next(&UpdateEvent::Found("0.5.2".into())), downloading);
        assert_eq!(downloading.next(&UpdateEvent::NotFound), downloading);
        assert_eq!(
            UpdateState::Idle.next(&UpdateEvent::Started),
            UpdateState::Idle,
            "知らせていない版はダウンロードしない"
        );
    }

    #[test]
    fn checks_a_minute_after_start_then_daily() {
        let start = 1_000_000_000;
        assert!(!is_due(start + FIRST_CHECK_DELAY_MS - 1, start, None, false));
        assert!(is_due(start + FIRST_CHECK_DELAY_MS, start, None, false));
        let last = start + FIRST_CHECK_DELAY_MS;
        assert!(!is_due(last + CHECK_INTERVAL_MS - 1, start, Some(last), false));
        assert!(is_due(last + CHECK_INTERVAL_MS, start, Some(last), false));
        // スリープで何日も止まっていても、戻ったときに 1 回だけ確かめればよい
        assert!(is_due(last + 3 * CHECK_INTERVAL_MS, start, Some(last), false));
        // 時計を戻したときに、ずっと確かめなくならない
        assert!(is_due(last - CHECK_INTERVAL_MS, start, Some(last), false));
    }

    #[test]
    fn failed_check_is_retried_soon() {
        // スリープから戻った直後は Wi-Fi がまだつながっておらず失敗しやすい。次を 24 時間後にすると、何日も知らせが出ない
        let start = 1_000_000_000;
        let last = start + FIRST_CHECK_DELAY_MS;
        assert!(!is_due(last + RETRY_AFTER_FAILURE_MS - 1, start, Some(last), true));
        assert!(is_due(last + RETRY_AFTER_FAILURE_MS, start, Some(last), true));
        const { assert!(RETRY_AFTER_FAILURE_MS < CHECK_INTERVAL_MS) };
    }

    #[test]
    fn serializes_for_the_page() {
        let json = serde_json::to_value(UpdateState::Downloading {
            version: "0.5.1".into(),
            percent: 40,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "downloading", "version": "0.5.1", "percent": 40 })
        );
        assert_eq!(
            serde_json::to_value(UpdateState::Idle).unwrap(),
            serde_json::json!({ "kind": "idle" })
        );
    }
}
