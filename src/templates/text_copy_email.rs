/// Text copy update email for event attendees.
///
/// Variables: event_image_cid (optional), text_copy, ticket_url (optional),
/// external_url (optional), unsubscribe_url
pub const HTML: &str = r#"<!DOCTYPE html>
<html>
  <body style="margin: 0; padding: 0; font-family: 'IBM Plex Sans', Arial, sans-serif; background-color: #ffffff; color: #000000;">
    <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
      {{event_image}}
      <p>{{text_copy}}</p>
      {{buttons}}
      <p>Best,</p>
      <p>Regards</p>
      <p style="margin-top: 30px;">
        <a href="{{unsubscribe_url}}" style="color: #000000; text-decoration: underline;">Unsubscribe</a>
      </p>
    </div>
  </body>
</html>"#;
