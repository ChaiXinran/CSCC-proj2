//! Expression parsing will use Pratt precedence rules.
//!
//! Keeping expression parsing in its own module lets one contributor extend
//! precedence handling without editing statement parsing.
