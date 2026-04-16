mod notify;
mod remind;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "notify", about = "macOS notifications with tmux context")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a notification
    Send {
        /// Notification message
        message: String,

        /// Custom title (default: "notify")
        #[arg(short, long, default_value = "notify")]
        title: String,

        /// If set, remove this one-shot reminder from crontab after sending
        #[arg(long, hide = true)]
        once: Option<String>,
    },

    /// Schedule a reminder notification
    Remind {
        /// Reminder message
        message: String,

        /// Cron expression (e.g. "0 10 * * 1-5")
        #[arg(short, long, required_unless_present = "delay")]
        cron: Option<String>,

        /// Fire once then auto-remove (implied by --in)
        #[arg(long, default_value_t = false)]
        once: bool,

        /// Duration from now (e.g. "30m", "2h", "5d"). Implies --once.
        #[arg(long = "in", id = "delay", required_unless_present = "cron")]
        delay: Option<String>,
    },

    /// List active reminders
    List,

    /// Remove a reminder by ID
    Remove {
        /// Reminder ID
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Send {
            message,
            title,
            once,
        } => {
            let tmux = notify::get_tmux_context();
            let subtitle = tmux.as_ref().map(|t| format!("tmux: {t}"));

            if let Err(e) =
                notify::send_notification(&title, &message, subtitle.as_deref())
            {
                eprintln!("error: {e}");
                std::process::exit(1);
            }

            // Clean up one-shot reminder if applicable
            if let Some(id) = once {
                if let Err(e) = remind::remove_once_reminder(&id) {
                    eprintln!("warning: failed to remove one-shot reminder: {e}");
                }
            }
        }

        Commands::Remind {
            message,
            cron,
            once,
            delay,
        } => {
            let (cron_expr, is_once) = if let Some(duration) = delay {
                match remind::duration_to_cron(&duration) {
                    Ok(expr) => (expr, true),
                    Err(e) => {
                        eprintln!("error: {e}");
                        std::process::exit(1);
                    }
                }
            } else {
                (cron.unwrap(), once)
            };

            match remind::add_reminder(&message, &cron_expr, is_once) {
                Ok(id) => println!("reminder set: {id}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }

        Commands::List => {
            let reminders = remind::list_reminders();
            if reminders.is_empty() {
                println!("no active reminders");
            } else {
                println!("{:<10} {:<20} {}", "ID", "SCHEDULE", "MESSAGE");
                for r in &reminders {
                    println!("{:<10} {:<20} {}", r.id, r.cron, r.message);
                }
            }
        }

        Commands::Remove { id } => match remind::remove_reminder(&id) {
            Ok(true) => println!("removed: {id}"),
            Ok(false) => {
                eprintln!("not found: {id}");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        },
    }
}
