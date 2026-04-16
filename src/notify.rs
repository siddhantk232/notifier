use std::process::Command;

pub struct TmuxContext {
    pub session: String,
    pub window: String,
}

impl std::fmt::Display for TmuxContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.session, self.window)
    }
}

pub fn get_tmux_context() -> Option<TmuxContext> {
    // Check if we're inside tmux
    if std::env::var("TMUX").is_err() {
        return None;
    }

    let session = Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })?;

    let window = Command::new("tmux")
        .args(["display-message", "-p", "#W"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })?;

    Some(TmuxContext { session, window })
}

pub fn send_notification(title: &str, message: &str, subtitle: Option<&str>) -> Result<(), String> {
    let bundle = mac_notification_sys::get_bundle_identifier_or_default("com.apple.Terminal");
    mac_notification_sys::set_application(&bundle).map_err(|e| format!("Failed to set app: {e}"))?;

    let mut notification = mac_notification_sys::Notification::new();
    notification.title(title);
    notification.message(message);
    notification.sound("default");

    if let Some(sub) = subtitle {
        notification.subtitle(sub);
    }

    notification
        .send()
        .map(|_| ())
        .map_err(|e| format!("Failed to send notification: {e}"))
}
