use tabled::{
    settings::{
        format::Format,
        object::{Columns, Rows},
        Alignment, Modify, Style,
    },
    Table,
};

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

    /// Apply theme for status tables - curved style with uppercase headers
    pub fn apply_status(table: Table) -> Table {
        Self::apply_default(table)
    }

    /// Apply theme for wide tables - clean spacing for wide output with curved borders
    pub fn apply_wide(table: Table) -> Table {
        Self::apply_default(table)
    }

    /// Apply theme with color coding for status indicators (optional, can be enabled later)
    #[allow(dead_code)]
    pub fn apply_with_colors(mut table: Table) -> Table {
        table = Self::apply_default(table);
        // Color coding can be added here when needed
        table
    }
}
