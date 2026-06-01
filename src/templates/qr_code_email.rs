/// QR code email for event attendees.
///
/// Variables: event_name, full_name, event_description, event_date,
/// qr_image_cid, event_image_cid (optional), unsubscribe_url
pub const HTML: &str = r#"<!DOCTYPE html>
<html>
  <body style="margin: 0; padding: 0; font-family: 'IBM Plex Sans', Arial, sans-serif; background-color: #ffffff; color: #000000;">
    <div style="max-width: 600px; margin: 0 auto; padding: 20px;">
      {{event_image}}
      <h1 style="font-size: 24px; margin-bottom: 16px;">QR Code for {{event_name}}</h1>
      <div style="margin: 20px 0;">
        <img src="cid:{{qr_image_cid}}" alt="QR Code" style="width: 200px; height: 200px;" />
      </div>
      <p>{{full_name}},</p>
      <p>{{event_name}}</p>
      <p>{{event_description}}</p>
      <p>{{event_date}}</p>
      <p>Best,</p>
      <p>Regards</p>
      <p style="margin-top: 30px;">
        <a href="{{unsubscribe_url}}" style="color: #000000; text-decoration: underline;">Unsubscribe</a>
      </p>
    </div>
  </body>
</html>"#;
