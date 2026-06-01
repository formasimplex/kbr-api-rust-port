/// Reset trigger email for password reset.
///
/// Variables: full_name, confirmation_url
pub const HTML: &str = r#"<!DOCTYPE html>
<html>
  <body style="margin: 0; padding: 0; font-family: 'IBM Plex Sans', Arial, sans-serif; background-color: #ffffff; color: #000000;">
    <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
      <h1 style="font-size: 24px; margin-bottom: 16px;">Reset Your Password</h1>
      <p>Hello {{full_name}},</p>
      <p>You have requested to reset your password. Click the link below to set a new password:</p>
      <p style="margin: 20px 0;">
        <a href="{{confirmation_url}}" style="background-color: #000000; color: #ffffff; padding: 12px 24px; text-decoration: none; border-radius: 4px;">Reset Password</a>
      </p>
      <p>If you did not request this, please ignore this email.</p>
      <p>Best,</p>
      <p>Regards</p>
    </div>
  </body>
</html>"#;
