use colored::*;

/// Print a bio-binary banner
pub fn banner(name: &str, layer: &str, emoji: &str) {
    println!("{}", format!("╔══════════════════════════════════════════╗").bright_cyan());
    println!("{}", format!("║  {} {} ", emoji, name).bright_cyan());
    println!("{}", format!("║  Layer: {}", layer).cyan());
    println!("{}", format!("╚══════════════════════════════════════════╝").bright_cyan());
    println!();
}

/// Print a section header
pub fn section(title: &str) {
    println!("  {} {}", "▸".bright_yellow(), title.bold());
}

/// Print a key-value pair
pub fn kv(key: &str, value: &str) {
    println!("    {}: {}", key.dimmed(), value.bright_white());
}

/// Print a status line with color based on level
pub fn status(label: &str, value: f64, unit: &str) {
    let colored_val = if value > 80.0 {
        format!("{:.1}{}", value, unit).bright_red()
    } else if value > 50.0 {
        format!("{:.1}{}", value, unit).bright_yellow()
    } else {
        format!("{:.1}{}", value, unit).bright_green()
    };
    println!("    {}: {}", label.dimmed(), colored_val);
}

/// Print a progress bar
pub fn progress_bar(label: &str, value: f64, max: f64) {
    let pct = (value / max * 100.0).min(100.0);
    let filled = (pct / 5.0) as usize;
    let empty = 20 - filled;
    let bar_color = if pct > 80.0 {
        format!("{}{}", "█".repeat(filled), "░".repeat(empty)).bright_red()
    } else if pct > 50.0 {
        format!("{}{}", "█".repeat(filled), "░".repeat(empty)).bright_yellow()
    } else {
        format!("{}{}", "█".repeat(filled), "░".repeat(empty)).bright_green()
    };
    println!("    {} [{}] {:.1}%", label.dimmed(), bar_color, pct);
}

/// Print success message
pub fn success(msg: &str) {
    println!("  {} {}", "[OK]".bright_green(), msg);
}

/// Print warning
pub fn warn(msg: &str) {
    println!("  {} {}", "[WARN]".bright_yellow(), msg);
}

/// Print error
pub fn error(msg: &str) {
    println!("  {} {}", "[ERR]".bright_red(), msg);
}


/// Final summary line
pub fn summary(name: &str, status_text: &str) {
    println!();
    println!("  {} {} :: {}", "⟫".bright_cyan(), name.bold(), status_text.bright_green());
    println!();
}
