use url::Url;

#[derive(Clone)]
pub struct DashLocation {
    dash_url: Url,
    mpd_name: String,
}

impl DashLocation {    
    pub fn new(server_url: &str, dash_path: &str, mpd_name: &str) -> Result<Self, url::ParseError> {
        let dash_url = Url::parse(server_url)?.join(dash_path)?;
        Ok(Self { dash_url, mpd_name: mpd_name.to_string() })
    }
    
    pub fn mpd_url(&self) -> Url {
        // assume that no problems can occur at that point
        self.dash_url.join(&self.mpd_name).unwrap()
    }

    pub fn segment_url(&self, segment_path: &str) -> Url {
        // assume that no problems can occur at that point
        self.dash_url.join(segment_path).unwrap()
    }
}
