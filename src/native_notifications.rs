//! Bounded macOS notification authorization bridge, without notification delivery.
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::AsyncReadExt;

/// Authorization reported by the operating system helper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationStatus {
    /// The user has not chosen yet.
    NotDetermined,
    /// The user denied authorization.
    Denied,
    /// Alerts are authorized.
    Authorized,
    /// Quiet alerts are authorized.
    Provisional,
    /// Temporary authorization is available.
    Ephemeral,
    /// The operating system cannot report authorization.
    Unavailable,
}
impl AuthorizationStatus {
    /// Vietnamese label for the Settings panel.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotDetermined => "Chưa yêu cầu quyền",
            Self::Denied => "Đã từ chối. Bạn có thể đổi trong Cài đặt hệ thống macOS.",
            Self::Authorized => "Đã cấp quyền",
            Self::Provisional => "Được phép thông báo im lặng",
            Self::Ephemeral => "Được cấp quyền tạm thời",
            Self::Unavailable => "Trạng thái quyền chưa khả dụng",
        }
    }
}
/// Bounded helper failures; raw helper output never reaches the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationError {
    /// Permission requests require an explicit user gesture.
    ExplicitUserActionRequired,
    /// This platform or installation has no executable helper.
    HelperUnavailable,
    /// The helper exceeded its deadline.
    Timeout,
    /// The helper exited unsuccessfully or returned invalid/oversized output.
    InvalidResponse,
    /// The child pipe could not be read.
    Io,
}
impl std::fmt::Display for NotificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ExplicitUserActionRequired => "Hãy bấm nút yêu cầu quyền để tiếp tục.",
            Self::HelperUnavailable => "Chưa tìm thấy trợ giúp quyền thông báo macOS.",
            Self::Timeout => "Đọc quyền thông báo quá thời gian. Hãy thử lại.",
            Self::InvalidResponse => "Trợ giúp quyền thông báo trả kết quả không hợp lệ.",
            Self::Io => "Lỗi đọc kết quả quyền thông báo. Hãy thử lại.",
        })
    }
}
impl std::error::Error for NotificationError {}

fn packaged_helper(executable: &Path) -> Option<PathBuf> {
    Some(
        executable
            .parent()?
            .parent()?
            .join("Helpers/DailyNotifier.app/Contents/MacOS/DailyNotifier"),
    )
}
fn helper_path() -> PathBuf {
    let packaged = std::env::current_exe()
        .ok()
        .and_then(|path| packaged_helper(&path));
    packaged.filter(|path| path.is_file()).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("native/macos/build/DailyNotifier.app/Contents/MacOS/DailyNotifier")
    })
}
/// Spawn the helper, retrying briefly on `ETXTBSY`: a write handle to a freshly written executable
/// can still be held by a concurrently forked child that has not reached `exec` yet.
async fn spawn_helper(
    path: &Path,
    command: &str,
) -> Result<tokio::process::Child, NotificationError> {
    let mut attempt = 0;
    loop {
        match tokio::process::Command::new(path)
            .arg(command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(child) => return Ok(child),
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && attempt < 5 => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(10 * attempt)).await;
            }
            Err(_) => return Err(NotificationError::HelperUnavailable),
        }
    }
}
async fn invoke(
    path: &Path,
    command: &str,
    deadline: Duration,
) -> Result<AuthorizationStatus, NotificationError> {
    let mut child = spawn_helper(path, command).await?;
    let operation = async {
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .ok_or(NotificationError::Io)?
            .take(4097)
            .read_to_end(&mut output)
            .await
            .map_err(|_| NotificationError::Io)?;
        if output.len() > 4096 {
            return Err(NotificationError::InvalidResponse);
        }
        if !child
            .wait()
            .await
            .map_err(|_| NotificationError::Io)?
            .success()
        {
            return Err(NotificationError::InvalidResponse);
        }
        parse_status(std::str::from_utf8(&output).map_err(|_| NotificationError::InvalidResponse)?)
    };
    tokio::time::timeout(deadline, operation)
        .await
        .map_err(|_| NotificationError::Timeout)?
}
fn parse_status(output: &str) -> Result<AuthorizationStatus, NotificationError> {
    match output.trim() {
        "notDetermined" => Ok(AuthorizationStatus::NotDetermined),
        "denied" => Ok(AuthorizationStatus::Denied),
        "authorized" => Ok(AuthorizationStatus::Authorized),
        "provisional" => Ok(AuthorizationStatus::Provisional),
        "ephemeral" => Ok(AuthorizationStatus::Ephemeral),
        "unavailable" => Ok(AuthorizationStatus::Unavailable),
        _ => Err(NotificationError::InvalidResponse),
    }
}
async fn platform_invoke(
    command: &str,
    deadline: Duration,
) -> Result<AuthorizationStatus, NotificationError> {
    if !cfg!(target_os = "macos") {
        return Err(NotificationError::HelperUnavailable);
    }
    invoke(&helper_path(), command, deadline).await
}
/// Read authorization without asking for permission or posting a notification.
///
/// # Errors
/// Returns a typed error for unsupported platforms, missing helpers, deadlines or invalid output.
pub async fn status() -> Result<AuthorizationStatus, NotificationError> {
    platform_invoke("--status", Duration::from_secs(10)).await
}
/// Request authorization only after the user explicitly clicks the permission button.
///
/// # Errors
/// Returns `ExplicitUserActionRequired` before spawning when the gesture is absent,
/// or a typed helper error otherwise. This function never posts notifications.
pub async fn request_if_user_action(
    explicit_user_action: bool,
) -> Result<AuthorizationStatus, NotificationError> {
    if !explicit_user_action {
        return Err(NotificationError::ExplicitUserActionRequired);
    }
    platform_invoke("--request", Duration::from_secs(120)).await
}

/// Deliver a native desktop notification for incoming messages.
///
/// On Linux: invokes `notify-send` asynchronously if available.
/// On macOS: uses AppleScript notification.
/// On Windows: fires a PowerShell balloon notification.
pub fn deliver_desktop_notification(title: &str, body: &str) {
    let clean_title = title.replace('\n', " ").replace('\r', "");
    let clean_body = body.replace('\r', "");

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("notify-send")
            .args([
                "-a",
                "ctdozlite",
                "-i",
                "dialog-information",
                "-c",
                "im.received",
                &clean_title,
                &clean_body,
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let esc_title = clean_title.replace('\\', "\\\\").replace('"', "\\\"");
        let esc_body = clean_body.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("display notification \"{esc_body}\" with title \"{esc_title}\"");
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        let esc_title = clean_title.replace('\'', "''");
        let esc_body = clean_body.replace('\'', "''");
        let ps_script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $n = New-Object System.Windows.Forms.NotifyIcon; \
             $n.Icon = [System.Drawing.SystemIcons]::Information; \
             $n.BalloonTipTitle = '{esc_title}'; \
             $n.BalloonTipText = '{esc_body}'; \
             $n.Visible = $true; \
             $n.ShowBalloonTip(4000); \
             Start-Sleep -Seconds 4; \
             $n.Dispose();"
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

/// UI-owned sequence gate: at most one request is pending; stale completions are ignored.
#[derive(Default)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) struct AuthorizationState {
    next: u64,
    pub(crate) pending: Option<u64>,
    pub(crate) result: Option<Result<AuthorizationStatus, NotificationError>>,
}
impl AuthorizationState {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn begin(&mut self) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.next = self.next.checked_add(1)?;
        self.pending = Some(self.next);
        Some(self.next)
    }
    pub(crate) fn finish(
        &mut self,
        id: u64,
        result: Result<AuthorizationStatus, NotificationError>,
    ) {
        if self.pending == Some(id) {
            self.pending = None;
            self.result = Some(result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deliver_desktop_notification_does_not_panic_on_any_input() {
        deliver_desktop_notification("Sender\nName", "Hello\r\nWorld! <script>");
        deliver_desktop_notification("", "");
    }

    #[test]
    fn parser_accepts_all_statuses_and_rejects_unknown() {
        for (raw, expected) in [
            ("notDetermined", AuthorizationStatus::NotDetermined),
            ("denied", AuthorizationStatus::Denied),
            ("authorized", AuthorizationStatus::Authorized),
            ("provisional", AuthorizationStatus::Provisional),
            ("ephemeral", AuthorizationStatus::Ephemeral),
            ("unavailable", AuthorizationStatus::Unavailable),
        ] {
            assert_eq!(parse_status(&format!("{raw}\n")), Ok(expected));
        }
        for invalid in ["", "unknown", "authorized\ndenied", "submitted"] {
            assert_eq!(
                parse_status(invalid),
                Err(NotificationError::InvalidResponse)
            );
        }
    }

    #[tokio::test]
    async fn permission_requires_explicit_action_before_spawn() {
        assert_eq!(
            request_if_user_action(false).await,
            Err(NotificationError::ExplicitUserActionRequired)
        );
    }

    #[test]
    fn state_prevents_duplicates_and_ignores_stale_results_with_retry() {
        let mut state = AuthorizationState::default();
        let first = state.begin().unwrap();
        assert_eq!(state.begin(), None);
        state.finish(first + 1, Ok(AuthorizationStatus::Authorized));
        assert_eq!(state.pending, Some(first));
        state.finish(first, Err(NotificationError::Timeout));
        assert_eq!(state.result, Some(Err(NotificationError::Timeout)));
        let second = state.begin().unwrap();
        state.finish(first, Ok(AuthorizationStatus::Denied));
        assert_eq!(state.pending, Some(second));
        state.finish(second, Ok(AuthorizationStatus::Authorized));
        assert_eq!(state.result, Some(Ok(AuthorizationStatus::Authorized)));
    }

    #[test]
    fn every_helper_error_clears_loading_and_allows_retry() {
        let mut state = AuthorizationState::default();
        for error in [
            NotificationError::HelperUnavailable,
            NotificationError::Timeout,
            NotificationError::InvalidResponse,
            NotificationError::Io,
            NotificationError::ExplicitUserActionRequired,
        ] {
            let id = state.begin().unwrap();
            state.finish(id, Err(error));
            assert_eq!(state.pending, None);
            assert_eq!(state.result, Some(Err(error)));
        }
        assert!(state.begin().is_some());
    }

    #[test]
    fn packaged_path_relocates_with_parent_app() {
        assert_eq!(packaged_helper(Path::new("/tmp/Relocated.app/Contents/MacOS/zalo-rust")), Some(PathBuf::from("/tmp/Relocated.app/Contents/Helpers/DailyNotifier.app/Contents/MacOS/DailyNotifier")));
    }

    #[cfg(unix)]
    struct Fixture(PathBuf);
    #[cfg(unix)]
    impl Fixture {
        fn new(body: &str) -> Self {
            use std::os::unix::fs::PermissionsExt;
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "ctdozlite-notification-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            let path = dir.join("helper");
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self(dir)
        }
        async fn run(&self, command: &str) -> Result<AuthorizationStatus, NotificationError> {
            invoke(&self.0.join("helper"), command, Duration::from_secs(3)).await
        }
    }
    #[cfg(unix)]
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mock_executable_valid_denied_and_exact_args() {
        let fixture = Fixture::new("printf '%s\\n' \"$@\" >> \"$0.args\"; case \"$1\" in --status) echo denied;; --request) echo authorized;; *) exit 9;; esac");
        assert_eq!(
            fixture.run("--status").await,
            Ok(AuthorizationStatus::Denied)
        );
        assert_eq!(
            fixture.run("--request").await,
            Ok(AuthorizationStatus::Authorized)
        );
        assert_eq!(
            std::fs::read_to_string(fixture.0.join("helper.args")).unwrap(),
            "--status\n--request\n"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mock_executable_rejects_malformed_utf8_oversize_and_failed_exit() {
        for body in [
            "echo bogus".to_owned(),
            "printf '\\377'".to_owned(),
            format!("printf '{}'", "x".repeat(4097)),
            "echo authorized; exit 1".to_owned(),
        ] {
            assert_eq!(
                Fixture::new(&body).run("--status").await,
                Err(NotificationError::InvalidResponse),
                "{body}"
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mock_executable_timeout_and_missing_helper() {
        assert_eq!(
            invoke(
                &Fixture::new("exec sleep 5").0.join("helper"),
                "--status",
                Duration::from_millis(100)
            )
            .await,
            Err(NotificationError::Timeout)
        );
        let fixture = Fixture::new("echo authorized");
        std::fs::remove_file(fixture.0.join("helper")).unwrap();
        assert_eq!(
            fixture.run("--status").await,
            Err(NotificationError::HelperUnavailable)
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn unsupported_platform_does_not_launch_helper() {
        assert_eq!(status().await, Err(NotificationError::HelperUnavailable));
        assert_eq!(
            request_if_user_action(true).await,
            Err(NotificationError::HelperUnavailable)
        );
    }
}
