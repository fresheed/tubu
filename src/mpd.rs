use std::{error::Error, fmt::Display};

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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    VIDEO,
    AUDIO,
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::VIDEO => write!(f, "video"),
            ContentType::AUDIO => write!(f, "audio"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SegmentTimeline {
    #[serde(rename = "$text")]
    pub text: Option<String>,
    #[serde(rename = "S")]
    pub timeline: Vec<TimelineEntry>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RawTimelineEntry {
    #[serde(rename = "@t")]
    pub timestamp: Option<usize>,
    #[serde(rename = "@d")]
    pub duration: usize,
    #[serde(rename = "@r")]
    pub repeats: Option<usize>,
}

 #[derive(Serialize, Deserialize, Debug, Clone)]
 #[serde(from = "RawTimelineEntry")]
 pub enum TimelineEntry {
    RepeatedEntry { 
        timestamp: Option<usize>,
        duration: usize, 
        extra_repeats: usize
    },
    SingleEntry { 
        timestamp: Option<usize>,
        duration: usize 
    },
}

impl From<RawTimelineEntry> for TimelineEntry {
    fn from(raw: RawTimelineEntry) -> Self {
        if let Some(repeats) = raw.repeats {
            Self::RepeatedEntry { timestamp: raw.timestamp, duration: raw.duration,
                extra_repeats: repeats }
        } else {
            Self::SingleEntry { timestamp: raw.timestamp, duration: raw.duration }
        }
    }
}

impl TimelineEntry {    
    fn iter(&self) -> TEIterator<'_> {
        TEIterator { te: self, cur_index: 0 }
    }
}

// Incomplete stub.
// Only used for a dummy iterator over timeline entries
pub struct Segment {
    // name: String,
}

struct TEIterator<'a> {
    te: &'a TimelineEntry,
    cur_index: usize,
}

impl<'a> Iterator for TEIterator<'a> {
    type Item = Segment;

    fn next(&mut self) -> Option<Self::Item> {
        let num_segs = match self.te {
            TimelineEntry::RepeatedEntry { extra_repeats, .. } => 1 + extra_repeats,
            TimelineEntry::SingleEntry { .. } => 1,
        };
        if self.cur_index < num_segs {
            self.cur_index += 1;
            Some(Segment {})
        } else {
            None
        }
    }
}

impl AdaptationSet {

    // Lists the full names of segments, starting from the initialization one.
    // Currently only substitutes a limited set of placeholders
    pub fn segment_names_iterator(&self) -> impl Iterator<Item=String> {
        let init_name_tpl = &self.representation.segment_template.initialization;
        let base_name_tpl = &self.representation.segment_template.media;
        
        let init_segment = std::iter::once(self.subst_placeholers(init_name_tpl, 0));
        let media_segments = 
            self.representation.segment_template.segment_timeline.timeline
            .iter().flat_map(|i| { i.iter() })
            .enumerate().map(|(i, _e)| {
                // count for media segments starts from 1
                self.subst_placeholers(base_name_tpl, i + 1)
            });        
        init_segment.chain(media_segments)
    }

    fn subst_placeholers(&self, s: &str, index: usize) -> String {
        // init segment:  init-$RepresentationID$.m4s
        // media segment: chunk-$RepresentationID$-$Number%05d$.m4s
        // the 5-padding is currently hardcoded
        let padded_index = format!("{index:05}");
        s.replace("$RepresentationID$", &self.representation.id)
            .replace("$Number%05d$", &padded_index)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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