/// Sign-up trigger email for user/artist registration.
///
/// Variables: role, confirm_url
pub const HTML: &str = r#"<!DOCTYPE html>
<html>
  <body style="margin: 0; padding: 0; font-family: 'IBM Plex Sans', Arial, sans-serif; background-color: #ffffff; color: #000000;">
    <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
      <h1 style="font-size: 24px; margin-bottom: 16px;">Confirm Your {{role}} Account</h1>
      <p>Click the link below to confirm your {{role}} registration:</p>
      <div style="margin: 30px 0;">
        <a href="{{confirm_url}}" style="display: inline-block; padding: 12px 24px; background-color: #000000; color: #ffffff; text-decoration: none; border-radius: 6px; font-weight: bold;">Confirm {{role}}</a>
      </div>
      <p>This link will expire in 1 day.</p>
    </div>
  </body>
</html>"#;
