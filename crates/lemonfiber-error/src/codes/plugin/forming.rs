//! The `PLUGIN` code of a plugin naming a form the stack does not declare.

codes! {
    /// Raised when a plugin's manifest names a form the stack does not declare.
    UNDECLARED_FORM = "PLUGIN-52" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A plugin's services join the forms its manifest names, and it names one or \
            more this stack does not declare, so they would join nothing there. Nothing was \
            written; the message names each form and the ones the stack declares.",
        remedy: "Install a version of the plugin whose forms are among the ones this stack \
            declares, which lemonfiber forms lists.",
        household: fault,
    }
}
