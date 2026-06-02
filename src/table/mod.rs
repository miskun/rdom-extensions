//! Table components built on rdom-tui's native `<table>` elements.
//!
//! - [`virtual_table`] — a windowed table that materializes only the
//!   visible row slice, for large datasets.

pub mod virtual_table;

pub use virtual_table::{Column, VirtualTable, VirtualTableView};
