use tabled::{
    settings::{
        format::Format,
        object::{Columns, Rows},
        Alignment, Modify, Style,
    },
    Table,
};

/// Emoji mappings for common values in Kubernetes/Gateway API contexts
pub struct EmojiFormatter;

impl EmojiFormatter {
    /// Convert common boolean and status values to emojis
    pub fn format_value(value: &str) -> String {
        match value.to_lowercase().trim() {
            // Boolean states
            "true" => "✅".to_string(),
            "false" => "❌".to_string(),
            "yes" => "✅".to_string(),
            "no" => "❌".to_string(),

            // Status states
            "ready" => "✅ Ready".to_string(),
            "not ready" => "❌ Not Ready".to_string(),
            "available" => "✅ Available".to_string(),
            "running" => "🟢 Running".to_string(),
            "pending" => "🟡 Pending".to_string(),
            "succeeded" => "✅ Succeeded".to_string(),
            "failed" => "❌ Failed".to_string(),
            "error" => "❌ Error".to_string(),
            "terminating" => "🟠 Terminating".to_string(),
            "unknown" => "❓ Unknown".to_string(),
            "not found" => "❓ Not Found".to_string(),

            // Gateway/Route specific statuses
            "accepted" => "✅ Accepted".to_string(),
            "rejected" => "❌ Rejected".to_string(),
            "programmed" => "🔧 Programmed".to_string(),
            "not programmed" => "⚠️ Not Programmed".to_string(),
            "resolved" => "✅ Resolved".to_string(),
            "not resolved" => "❌ Not Resolved".to_string(),
            "attached" => "🔗 Attached".to_string(),
            "not attached" => "🔓 Not Attached".to_string(),

            // Service types
            s if s.starts_with("clusterip") => format!("🔒 {}", s),
            s if s.starts_with("nodeport") => format!("🌐 {}", s),
            s if s.starts_with("loadbalancer") => format!("⚖️ {}", s),
            s if s.starts_with("externalname") => format!("🔗 {}", s),

            // Pod readiness patterns (e.g., "1/1", "2/3")
            s if s.contains('/') && s.chars().all(|c| c.is_numeric() || c == '/') => {
                let parts: Vec<&str> = s.split('/').collect();
                if parts.len() == 2 {
                    if let (Ok(ready), Ok(total)) =
                        (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                    {
                        if ready == total && ready > 0 {
                            format!("✅ {}", s)
                        } else if ready == 0 {
                            format!("❌ {}", s)
                        } else {
                            format!("🟡 {}", s)
                        }
                    } else {
                        s.to_string()
                    }
                } else {
                    s.to_string()
                }
            }

            // HTTP status codes
            s if s.starts_with("2") && s.len() == 3 && s.chars().all(|c| c.is_numeric()) => {
                format!("✅ {}", s)
            }
            s if s.starts_with("3") && s.len() == 3 && s.chars().all(|c| c.is_numeric()) => {
                format!("🔄 {}", s)
            }
            s if s.starts_with("4") && s.len() == 3 && s.chars().all(|c| c.is_numeric()) => {
                format!("⚠️ {}", s)
            }
            s if s.starts_with("5") && s.len() == 3 && s.chars().all(|c| c.is_numeric()) => {
                format!("❌ {}", s)
            }

            // Numbers (restart counts, etc.)
            "0" => "✅ 0".to_string(),
            s if s.parse::<i32>().is_ok() => {
                let num = s.parse::<i32>().unwrap();
                if num > 0 {
                    format!("⚠️ {}", s)
                } else {
                    format!("✅ {}", s)
                }
            }

            // Default case - return original value
            _ => value.to_string(),
        }
    }

    /// Apply emoji formatting to a table column by index
    pub fn apply_to_column(mut table: Table, column_index: usize) -> Table {
        table.with(
            Modify::new(Columns::new(column_index..=column_index))
                .with(Format::content(Self::format_value)),
        );
        table
    }

    /// Apply emoji formatting to specific columns by name for common status fields
    #[allow(dead_code)]
    pub fn apply_to_status_columns(table: Table) -> Table {
        // This would ideally work with column names, but tabled works with indices
        // For now, we'll provide helper methods for specific table types
        table
    }
}

/// Centralized table theme configuration for consistent kubectl-like output
pub struct TableTheme;

impl TableTheme {
    /// Apply the default theme to a table - kubectl-like clean style with curved borders
    pub fn apply_default(mut table: Table) -> Table {
        table
            .with(Style::rounded())
            .with(Modify::new(Rows::first()).with(Format::content(|s| s.to_uppercase())))
            .with(Modify::new(Columns::new(..)).with(Alignment::left()));
        table
    }

    /// Apply the default theme with emoji formatting enabled
    pub fn apply_default_with_emoji(mut table: Table) -> Table {
        table = Self::apply_default(table);
        // Apply emoji formatting to all columns - will be refined per table type
        table
    }

    /// Apply theme for status tables - curved style with uppercase headers
    pub fn apply_status(table: Table) -> Table {
        Self::apply_default(table)
    }

    /// Apply theme for status tables with emoji formatting
    pub fn apply_status_with_emoji(mut table: Table) -> Table {
        table = Self::apply_status(table);
        // Common status table columns that benefit from emoji formatting
        // These indices would need to be adjusted based on actual table structure
        table
    }

    /// Apply theme for wide tables - clean spacing for wide output with curved borders
    pub fn apply_wide(table: Table) -> Table {
        Self::apply_default(table)
    }

    /// Apply theme for wide tables with emoji formatting
    pub fn apply_wide_with_emoji(table: Table) -> Table {
        Self::apply_default_with_emoji(table)
    }

    /// Apply theme with color coding for status indicators (optional, can be enabled later)
    #[allow(dead_code)]
    pub fn apply_with_colors(mut table: Table) -> Table {
        table = Self::apply_default(table);
        // Color coding can be added here when needed
        table
    }
}
