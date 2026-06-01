/// Prospect welcome email for new artist prospects.
///
/// Variables: full_name
pub const HTML: &str = r#"<!DOCTYPE html>
<html>
  <body style="margin: 0; padding: 0; font-family: 'IBM Plex Sans', Arial, sans-serif; background-color: #ffffff; color: #000000;">
    <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
      <h1 style="font-size: 24px; margin-bottom: 16px;">Welcome!</h1>
      <p>Hello {{full_name}},</p>
      <p>Welcome to the platform. We're excited to have you on board.</p>
      <p>Best,</p>
      <p>Regards</p>
    </div>
  </body>
</html>"#;
