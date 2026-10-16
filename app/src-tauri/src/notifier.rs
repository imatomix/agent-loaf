// tauri-plugin-notification は非推奨の NSUserNotificationCenter を使っており、
// macOS 26 ではエラーも返さずに通知が消えるため、UNUserNotificationCenter を直接使う

#[cfg(target_os = "macos")]
mod imp {
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{Arc, OnceLock};

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, AnyThread};
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
        UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse, UNNotificationSettings,
        UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "AgentLoafNotificationDelegate"]
        struct ForegroundPresenter;

        unsafe impl NSObjectProtocol for ForegroundPresenter {}

        // 一覧を開いている間はアプリが前面扱いになり、既定では通知が表示されないため
        unsafe impl UNUserNotificationCenterDelegate for ForegroundPresenter {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                completion_handler.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List
                    | UNNotificationPresentationOptions::Sound,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                completion_handler: &block2::DynBlock<dyn Fn()>,
            ) {
                let identifier = response.notification().request().identifier().to_string();
                if let (Some(session_id), Some(on_open)) = (identifier.strip_prefix(SESSION_PREFIX), ON_OPEN.get()) {
                    on_open(session_id.to_string());
                }
                completion_handler.call(());
            }
        }
    );

    // バンドルとして起動していないとき（tauri dev など）に currentNotificationCenter を呼ぶと例外で落ちる
    fn notification_center() -> Option<Retained<UNUserNotificationCenter>> {
        NSBundle::mainBundle().bundleIdentifier()?;
        Some(UNUserNotificationCenter::currentNotificationCenter())
    }

    const UNKNOWN: u8 = 0;
    const ALLOWED: u8 = 1;
    const DENIED: u8 = 2;
    // macOS の答えが来る前を「オフ」と区別しないと、起動直後に何もないのにバッジが出る
    static ACCESS: AtomicU8 = AtomicU8::new(UNKNOWN);
    const SESSION_PREFIX: &str = "session:";
    static ON_OPEN: OnceLock<Box<dyn Fn(String) + Send + Sync>> = OnceLock::new();

    pub fn notifications_allowed() -> Option<bool> {
        match ACCESS.load(Ordering::Relaxed) {
            ALLOWED => Some(true),
            DENIED => Some(false),
            _ => None,
        }
    }

    fn store_access(allowed: bool) {
        ACCESS.store(if allowed { ALLOWED } else { DENIED }, Ordering::Relaxed);
    }

    pub fn refresh_authorization(on_done: impl Fn() + Send + 'static) {
        let Some(center) = notification_center() else {
            // バンドルとして起動していない（tauri dev など）ときは通知を送らないので、案内やバッジも出さない
            store_access(true);
            on_done();
            return;
        };
        let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            let status = unsafe { settings.as_ref() }.authorizationStatus();
            let allowed = [
                UNAuthorizationStatus::Authorized,
                UNAuthorizationStatus::Provisional,
                UNAuthorizationStatus::Ephemeral,
            ]
            .contains(&status);
            store_access(allowed);
            on_done();
        });
        center.getNotificationSettingsWithCompletionHandler(&block);
    }

    pub fn init(on_change: impl Fn() + Send + Sync + 'static, on_open: impl Fn(String) + Send + Sync + 'static) {
        let _ = ON_OPEN.set(Box::new(on_open));
        let Some(center) = notification_center() else {
            eprintln!("not running as an app bundle; notifications are disabled");
            refresh_authorization(on_change);
            return;
        };
        let delegate: Retained<ForegroundPresenter> = unsafe { msg_send![ForegroundPresenter::alloc(), init] };
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // センターは delegate を弱参照で持つので、アプリが動いている間は解放させない
        std::mem::forget(delegate);

        let on_change = Arc::new(on_change);
        let on_answer = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            if let Some(error) = unsafe { error.as_ref() } {
                eprintln!(
                    "failed to request notification permission: {}",
                    error.localizedDescription()
                );
            } else if !granted.as_bool() {
                eprintln!("notifications are not allowed");
            }
            let on_change = on_change.clone();
            refresh_authorization(move || on_change());
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &on_answer,
        );
    }

    pub fn show(title: &str, body: &str, session_id: &str) {
        let Some(center) = notification_center() else { return };
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        // 識別子にセッション ID を入れると、押されたときにどのセッションか分かり、同じセッションの古い通知は置き換わる
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(&format!("{SESSION_PREFIX}{session_id}")),
            &content,
            None,
        );
        let on_added = RcBlock::new(|error: *mut NSError| {
            if let Some(error) = unsafe { error.as_ref() } {
                eprintln!("failed to send notification: {}", error.localizedDescription());
            }
        });
        center.addNotificationRequest_withCompletionHandler(&request, Some(&on_added));
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn init(_on_change: impl Fn() + Send + Sync + 'static, _on_open: impl Fn(String) + Send + Sync + 'static) {}
    pub fn show(_title: &str, _body: &str, _session_id: &str) {}
    pub fn notifications_allowed() -> Option<bool> {
        Some(true)
    }
    pub fn refresh_authorization(on_done: impl Fn() + Send + 'static) {
        on_done()
    }
}

pub use imp::{init, notifications_allowed, refresh_authorization, show};
