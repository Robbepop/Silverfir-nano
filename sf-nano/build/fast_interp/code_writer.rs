// Lightweight code generation helper.
// Wraps a String buffer with indentation tracking and convenience methods
// to replace repetitive push_str(&format!(...)) patterns across generators.

use std::fmt;

/// A code generation writer that tracks indentation.
pub struct CodeWriter {
    buf: String,
    indent: usize,
    indent_str: &'static str,
}

impl CodeWriter {
    /// Create a new CodeWriter with 4-space indentation.
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            indent: 0,
            indent_str: "    ",
        }
    }

    /// Append a pre-formatted line with current indentation.
    pub fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.buf.push_str(self.indent_str);
        }
        self.buf.push_str(text);
        self.buf.push('\n');
    }

    /// Append a formatted line with current indentation.
    pub fn fmt(&mut self, args: fmt::Arguments<'_>) {
        for _ in 0..self.indent {
            self.buf.push_str(self.indent_str);
        }
        self.buf.push_str(&args.to_string());
        self.buf.push('\n');
    }

    /// Append a blank line (no indentation).
    pub fn blank(&mut self) {
        self.buf.push('\n');
    }

    /// Increase indentation by one level.
    pub fn indent(&mut self) {
        self.indent += 1;
    }

    /// Decrease indentation by one level.
    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Consume the writer and return the generated code.
    pub fn finish(self) -> String {
        self.buf
    }
}

/// Convenience macro for calling `CodeWriter::fmt` with format_args.
#[macro_export]
macro_rules! wln {
    ($w:expr, $($arg:tt)*) => {
        $w.fmt(format_args!($($arg)*))
    };
}
