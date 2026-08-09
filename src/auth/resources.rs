pub const RESOURCES: ResourceNames = ResourceNames {
    album: "Album",
    song: "Song",
    artist: "Artist",
    campaign: "Campaign",
    kbr_event: "KBREvent",
    news: "News",
    user: "User",
};

pub struct ResourceNames {
    pub album: &'static str,
    pub song: &'static str,
    pub artist: &'static str,
    pub campaign: &'static str,
    pub kbr_event: &'static str,
    pub news: &'static str,
    pub user: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_constant_has_all_values() {
        assert_eq!(RESOURCES.album, "Album");
        assert_eq!(RESOURCES.song, "Song");
        assert_eq!(RESOURCES.artist, "Artist");
        assert_eq!(RESOURCES.campaign, "Campaign");
        assert_eq!(RESOURCES.kbr_event, "KBREvent");
        assert_eq!(RESOURCES.news, "News");
        assert_eq!(RESOURCES.user, "User");
    }
}
