codes! {
    /// Raised when no form was named.
    NO_FORM_NAMED = "FORM-1" {
        severity: Error,
        status: 400,
        since: "0.1.0",
        meaning: "No form was named, so there is nothing to start.",
        remedy: "Name a form, or list the ones this stack has with `lemonfiber forms`.",
    }
    /// Raised when a named form is not declared by the stack.
    NO_SUCH_FORM = "FORM-2" {
        severity: Error,
        status: 404,
        since: "0.1.0",
        meaning: "The stack declares no form by that name. Forms come from the stack rather than \
            from lemonfiber, so a stack of your own may name them differently.",
        remedy: "Use one of the names it offers. The message suggests the nearest match and \
            lists the rest.",
    }
    /// Raised when forms that cannot be combined are named together.
    FORMS_CONFLICT = "FORM-3" {
        severity: Error,
        status: 400,
        since: "0.1.0",
        meaning: "One of the forms you named has to run on its own — what it starts would \
            conflict with the others rather than add to them.",
        remedy: "Run that form by itself.",
    }
    /// Raised when narrowing leaves nothing to run.
    NOTHING_TO_RUN = "FORM-4" {
        severity: Warning,
        status: 500,
        since: "0.1.0",
        meaning: "Everything these forms would start needs a download provider, and none is \
            configured. Starting them would give you services that cannot fetch anything.",
        remedy: "Add a Usenet provider, or a VPN and a torrent client, with `lemonfiber setup`.",
    }
}
