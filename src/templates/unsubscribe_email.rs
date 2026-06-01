/// Unsubscribe confirmation email.
///
/// Variables: full_name, unsubscribe_url
pub const HTML: &str = r#"<!DOCTYPE html>
<html>
  <body style="margin: 0; padding: 0; font-family: 'IBM Plex Sans', Arial, sans-serif; background-color: #ffffff; color: #000000;">
    <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
      <h1 style="font-size: 24px; margin-bottom: 16px;">Confirm Unsubscribe</h1>
      <p>Hello {{full_name}},</p>
      <p>Click the link below to confirm your unsubscribe request:</p>
      <div style="margin: 30px 0;">
        <a href="{{unsubscribe_url}}" style="display: inline-block; padding: 12px 24px; background-color: #000000; color: #ffffff; text-decoration: none; border-radius: 6px; font-weight: bold;">Unsubscribe</a>
      </div>
      <p>If you did not request this, please ignore this email.</p>
      <p>Best,</p>
      <p>Regards</p>
    </div>
  </body>
</html>"#;
