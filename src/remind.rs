use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

const PLIST_PREFIX: &str = "com.notifier.reminder";

pub struct Reminder {
    pub id: String,
    pub cron: String,
    pub message: String,
}

fn launch_agents_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join("Library/LaunchAgents")
}

fn plist_path(id: &str) -> PathBuf {
    launch_agents_dir().join(format!("{PLIST_PREFIX}.{id}.plist"))
}

fn notify_binary_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "notify".to_string())
}

/// Parse a 5-field cron expression into a launchd StartCalendarInterval dict.
/// Supports: specific values, wildcards (*), comma-separated lists, and ranges (1-5).
/// Does NOT support step values (*/5) — use StartInterval for that.
fn cron_to_calendar_intervals(cron: &str) -> Result<Vec<String>, String> {
    let fields: Vec<&str> = cron.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(format!("expected 5 cron fields, got {}", fields.len()));
    }

    let minute = fields[0];
    let hour = fields[1];
    let day = fields[2];
    let month = fields[3];
    let weekday = fields[4];

    // Check for step values — convert to StartInterval instead
    if fields.iter().any(|f| f.contains('/')) {
        return Err("step values (*/N) not supported in calendar intervals; use a specific schedule".into());
    }

    // Expand each field into possible values
    let minutes = expand_field(minute, 0, 59)?;
    let hours = expand_field(hour, 0, 23)?;
    let days = expand_field(day, 1, 31)?;
    let months = expand_field(month, 1, 12)?;
    let weekdays = expand_field(weekday, 0, 6)?;

    // Generate one dict per combination (usually just one)
    let mut dicts = Vec::new();

    // For multiple weekdays/months we need separate dicts
    let weekday_vals = weekdays.unwrap_or_else(|| vec![]);
    let month_vals = months.unwrap_or_else(|| vec![]);
    let day_vals = days.unwrap_or_else(|| vec![]);
    let hour_vals = hours.unwrap_or_else(|| vec![]);
    let minute_vals = minutes.unwrap_or_else(|| vec![]);

    // If we have specific weekdays, generate a dict per weekday
    if !weekday_vals.is_empty() {
        for wd in &weekday_vals {
            let mut dict = "        <dict>\n".to_string();
            if !minute_vals.is_empty() {
                dict += &format!("            <key>Minute</key>\n            <integer>{}</integer>\n", minute_vals[0]);
            }
            if !hour_vals.is_empty() {
                dict += &format!("            <key>Hour</key>\n            <integer>{}</integer>\n", hour_vals[0]);
            }
            if !day_vals.is_empty() {
                dict += &format!("            <key>Day</key>\n            <integer>{}</integer>\n", day_vals[0]);
            }
            if !month_vals.is_empty() {
                dict += &format!("            <key>Month</key>\n            <integer>{}</integer>\n", month_vals[0]);
            }
            dict += &format!("            <key>Weekday</key>\n            <integer>{wd}</integer>\n");
            dict += "        </dict>";
            dicts.push(dict);
        }
    } else {
        let mut dict = "        <dict>\n".to_string();
        if !minute_vals.is_empty() {
            dict += &format!("            <key>Minute</key>\n            <integer>{}</integer>\n", minute_vals[0]);
        }
        if !hour_vals.is_empty() {
            dict += &format!("            <key>Hour</key>\n            <integer>{}</integer>\n", hour_vals[0]);
        }
        if !day_vals.is_empty() {
            dict += &format!("            <key>Day</key>\n            <integer>{}</integer>\n", day_vals[0]);
        }
        if !month_vals.is_empty() {
            dict += &format!("            <key>Month</key>\n            <integer>{}</integer>\n", month_vals[0]);
        }
        dict += "        </dict>";
        dicts.push(dict);
    }

    Ok(dicts)
}

/// Expand a cron field. Returns None for wildcard (*), Some(vec) for specific values.
fn expand_field(field: &str, min: u32, max: u32) -> Result<Option<Vec<u32>>, String> {
    if field == "*" {
        return Ok(None);
    }

    let mut values = Vec::new();
    for part in field.split(',') {
        if let Some((start, end)) = part.split_once('-') {
            let s: u32 = start.parse().map_err(|_| format!("invalid cron value: {start}"))?;
            let e: u32 = end.parse().map_err(|_| format!("invalid cron value: {end}"))?;
            if s < min || e > max || s > e {
                return Err(format!("range {s}-{e} out of bounds ({min}-{max})"));
            }
            values.extend(s..=e);
        } else {
            let v: u32 = part.parse().map_err(|_| format!("invalid cron value: {part}"))?;
            if v < min || v > max {
                return Err(format!("value {v} out of bounds ({min}-{max})"));
            }
            values.push(v);
        }
    }

    Ok(Some(values))
}

fn generate_plist(id: &str, message: &str, cron: &str, once: bool) -> Result<String, String> {
    let binary = notify_binary_path();
    let label = format!("{PLIST_PREFIX}.{id}");

    let calendar_dicts = cron_to_calendar_intervals(cron)?;
    let calendar_xml = calendar_dicts.join("\n");

    let once_args = if once {
        format!(
            "\n        <string>--once</string>\n        <string>{id}</string>"
        )
    } else {
        String::new()
    };

    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>send</string>
        <string>{message}</string>{once_args}
    </array>
    <key>StartCalendarInterval</key>
    <array>
{calendar_xml}
    </array>
    <key>StandardErrorPath</key>
    <string>/tmp/notifier.{id}.err</string>
</dict>
</plist>"#
    ))
}

pub fn add_reminder(message: &str, cron_expr: &str, once: bool) -> Result<String, String> {
    let id = Uuid::new_v4().to_string()[..8].to_string();
    let path = plist_path(&id);

    let plist = generate_plist(&id, message, cron_expr, once)?;
    fs::write(&path, &plist).map_err(|e| format!("failed to write plist: {e}"))?;

    // Load the agent
    let status = Command::new("launchctl")
        .args(["load", &path.to_string_lossy()])
        .status()
        .map_err(|e| format!("failed to run launchctl: {e}"))?;

    if !status.success() {
        // Clean up on failure
        let _ = fs::remove_file(&path);
        return Err("launchctl load failed".into());
    }

    Ok(id)
}

pub fn list_reminders() -> Vec<Reminder> {
    let dir = launch_agents_dir();
    let mut reminders = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return reminders,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(PLIST_PREFIX) || !name.ends_with(".plist") {
            continue;
        }

        // Extract ID from filename
        let id = name
            .strip_prefix(&format!("{PLIST_PREFIX}."))
            .and_then(|s| s.strip_suffix(".plist"))
            .unwrap_or("")
            .to_string();

        if let Ok(content) = fs::read_to_string(entry.path()) {
            // Extract message from ProgramArguments (the string after "send")
            let message = extract_plist_message(&content).unwrap_or_default();
            let cron = extract_plist_schedule(&content).unwrap_or_default();

            reminders.push(Reminder { id, cron, message });
        }
    }

    reminders
}

fn extract_plist_message(content: &str) -> Option<String> {
    // Find the <string>send</string> line, the next <string> is the message
    let mut found_send = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if found_send {
            if let Some(msg) = trimmed.strip_prefix("<string>") {
                if let Some(msg) = msg.strip_suffix("</string>") {
                    return Some(msg.to_string());
                }
            }
        }
        if trimmed == "<string>send</string>" {
            found_send = true;
        }
    }
    None
}

fn extract_plist_schedule(content: &str) -> Option<String> {
    // Reconstruct a cron-like string from StartCalendarInterval
    let mut minute = "*".to_string();
    let mut hour = "*".to_string();
    let mut day = "*".to_string();
    let mut month = "*".to_string();
    let mut weekdays: Vec<String> = Vec::new();

    let mut in_calendar = false;
    let mut current_key = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("StartCalendarInterval") {
            in_calendar = true;
            continue;
        }
        if in_calendar {
            if trimmed == "</array>" {
                break;
            }
            if let Some(key) = trimmed.strip_prefix("<key>").and_then(|s| s.strip_suffix("</key>")) {
                current_key = key.to_string();
            }
            if let Some(val) = trimmed
                .strip_prefix("<integer>")
                .and_then(|s| s.strip_suffix("</integer>"))
            {
                match current_key.as_str() {
                    "Minute" => minute = val.to_string(),
                    "Hour" => hour = val.to_string(),
                    "Day" => day = val.to_string(),
                    "Month" => month = val.to_string(),
                    "Weekday" => weekdays.push(val.to_string()),
                    _ => {}
                }
            }
        }
    }

    let wd = if weekdays.is_empty() {
        "*".to_string()
    } else {
        weekdays.join(",")
    };

    Some(format!("{minute} {hour} {day} {month} {wd}"))
}

pub fn remove_reminder(id: &str) -> Result<bool, String> {
    let path = plist_path(id);
    if !path.exists() {
        return Ok(false);
    }

    // Unload the agent
    let _ = Command::new("launchctl")
        .args(["unload", &path.to_string_lossy()])
        .status();

    fs::remove_file(&path).map_err(|e| format!("failed to remove plist: {e}"))?;
    Ok(true)
}

pub fn remove_once_reminder(id: &str) -> Result<(), String> {
    remove_reminder(id)?;
    Ok(())
}
