use std::error::Error;

use serde::{Deserialize, Serialize};

/*
    Generated from a sample manifest file using xml_schema_generator
*/

#[derive(Serialize, Deserialize, Debug)]
pub struct Mpd {
    #[serde(rename = "@xmlns:xsi")]
    pub xmlns_xsi: String,
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    #[serde(rename = "@xmlns:xlink")]
    pub xmlns_xlink: String,
    #[serde(rename = "@schemaLocation")]
    pub xsi_schema_location: String,
    #[serde(rename = "@profiles")]
    pub profiles: String,
    #[serde(rename = "@type")]
    pub mpd_type: String,
    #[serde(rename = "@mediaPresentationDuration")]
    pub media_presentation_duration: String,
    #[serde(rename = "@maxSegmentDuration")]
    pub max_segment_duration: String,
    #[serde(rename = "@minBufferTime")]
    pub min_buffer_time: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "ProgramInformation")]
    pub program_information: String,
    #[serde(rename = "ServiceDescription")]
    pub service_description: ServiceDescription,
    #[serde(rename = "Period")]
    pub period: Period,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServiceDescription {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Period {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@start")]
    pub start: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "AdaptationSet")]
    pub adaptation_set: Vec<AdaptationSet>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    VIDEO,
    AUDIO,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdaptationSet {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@contentType")]
    pub content_type: ContentType,
    #[serde(rename = "@startWithSAP")]
    pub start_with_sap: String,
    #[serde(rename = "@segmentAlignment")]
    pub segment_alignment: String,
    #[serde(rename = "@bitstreamSwitching")]
    pub bitstream_switching: String,
    #[serde(rename = "@frameRate")]
    pub frame_rate: Option<String>,
    #[serde(rename = "@maxWidth")]
    pub max_width: Option<String>,
    #[serde(rename = "@maxHeight")]
    pub max_height: Option<String>,
    #[serde(rename = "@par")]
    pub par: Option<String>,
    #[serde(rename = "@lang")]
    pub lang: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "Representation")]
    pub representation: Representation,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Representation {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@mimeType")]
    pub mime_type: String,
    #[serde(rename = "@codecs")]
    pub codecs: String,
    #[serde(rename = "@bandwidth")]
    pub bandwidth: String,
    #[serde(rename = "@width")]
    pub width: Option<String>,
    #[serde(rename = "@height")]
    pub height: Option<String>,
    #[serde(rename = "@sar")]
    pub sar: Option<String>,
    #[serde(rename = "@audioSamplingRate")]
    pub audio_sampling_rate: Option<String>,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "SegmentTemplate")]
    pub segment_template: SegmentTemplate,
    #[serde(rename = "AudioChannelConfiguration")]
    pub audio_channel_configuration: Option<AudioChannelConfiguration>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SegmentTemplate {
    #[serde(rename = "@timescale")]
    pub timescale: String,
    #[serde(rename = "@initialization")]
    pub initialization: String,
    #[serde(rename = "@media")]
    pub media: String,
    #[serde(rename = "@startNumber")]
    pub start_number: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "SegmentTimeline")]
    pub segment_timeline: SegmentTimeline,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SegmentTimeline {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "S")]
    pub s: Vec<S>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct S {
    #[serde(rename = "@t")]
    pub t: Option<String>,
    #[serde(rename = "@d")]
    pub d: String,
    #[serde(rename = "@r")]
    pub r: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AudioChannelConfiguration {
    #[serde(rename = "@schemeIdUri")]
    pub scheme_id_uri: String,
    #[serde(rename = "@value")]
    pub value: String,
}

#[derive(Debug)]
pub struct InvalidMpd {
    pub error: String,
}

impl std::fmt::Display for InvalidMpd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl Error for InvalidMpd { }

impl Mpd {

    // the rest of Mpd's public methods assume it's validated
    
    pub fn parse(s: &str) -> Result<Self, InvalidMpd> {
        let mpd0: Mpd = quick_xml::de::from_str(s)
            .map_err(|e| InvalidMpd { error: e.to_string()} )?;
        mpd0.validate()
    }

    fn validate(self) -> Result<Self, InvalidMpd> {
        if self.count_ct(ContentType::VIDEO) != 1 {
            return Err(InvalidMpd{ error: String::from("Manifest does not have exactly 1 'video' element") });
        };
        if self.count_ct(ContentType::AUDIO) != 1 {
            return Err(InvalidMpd{ error: String::from("Manifest does not have exactly 1 'audio' element") });
        };
        Ok(self)
    }

    fn count_ct(&self, ct: ContentType) -> usize {
        self.period.adaptation_set.iter()
            .filter(|s| s.content_type == ct).count()
    }

    // from now on, &self must be validated

    pub fn video_aset(&self) -> &AdaptationSet {
        self.period.adaptation_set.iter()
            .filter(|s| s.content_type == ContentType::VIDEO).next().unwrap()
    }

    pub fn audio_aset(&self) -> &AdaptationSet {
        self.period.adaptation_set.iter()
            .filter(|s| s.content_type == ContentType::AUDIO).next().unwrap()
    }
}