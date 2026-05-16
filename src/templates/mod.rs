pub mod qr_code_email;
pub mod sign_up_trigger_email;
pub mod text_copy_email;

/// Render a template string by replacing {{placeholder}} with values.
pub fn render(template: &str, vars: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_replaces_placeholders() {
        let tmpl = "Hello {{name}}, welcome to {{place}}!";
        assert_eq!(
            render(tmpl, &[("name", "Alice"), ("place", "KBR")]),
            "Hello Alice, welcome to KBR!"
        );
    }

    #[test]
    fn render_leaves_unknown_placeholders() {
        let tmpl = "Hello {{name}}, value is {{missing}}";
        assert_eq!(
            render(tmpl, &[("name", "Bob")]),
            "Hello Bob, value is {{missing}}"
        );
    }

    #[test]
    fn render_empty_vars_returns_template() {
        let tmpl = "static content";
        assert_eq!(render(tmpl, &[]), "static content");
    }
}
