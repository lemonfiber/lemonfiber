//! What setting the web interface's password says.
//!
//! The core asks for it at the keyboard rather than taking it from the command line: a
//! password given as an argument is in the shell's history and in the list of processes
//! this machine is running, and both outlive the moment it was typed.

use lemonfiber_core::PRODUCT;

/// What setting one says, as the lines an outcome renders to.
pub(crate) fn shown() -> crate::render::Lines {
    let mut lines = crate::render::Lines::default();
    for line in said() {
        lines.put(line);
    }
    lines
}

/// What setting one says, in order.
pub(crate) fn said() -> Vec<String> {
    vec![
        format!("A password is now set for {PRODUCT}'s web interface."),
        String::new(),
        "It is kept as something that proves an answer right, not as the answer. This machine \
         can tell a right password from a wrong one and cannot tell you what the right one is, \
         and neither can anybody who takes the file."
            .to_owned(),
        "Forgetting it means setting another one, not recovering this one.".to_owned(),
    ]
}
