//! A word put into a command the operator is handed to copy, quoted so the shell reads it
//! as the one word it is.

/// `word` quoted for a POSIX shell.
#[must_use]
pub fn posix(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// `word` quoted for `PowerShell`.
#[must_use]
pub fn powershell(word: &str) -> String {
    format!("'{}'", word.replace('\'', "''"))
}

#[cfg(test)]
mod tests;
