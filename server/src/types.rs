use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t")]
pub enum RawPayload {
    #[serde(rename = "pageview")]
    Pageview {
        s: String,
        p: String,
        ts: i64,
        #[serde(default)]
        r: Option<String>,
        #[serde(default)]
        d: Option<String>,
        #[serde(default)]
        v: Option<i32>,
    },
    #[serde(rename = "event")]
    Event {
        s: String,
        p: String,
        ts: i64,
        n: String,
        #[serde(default)]
        pr: Option<HashMap<String, serde_json::Value>>,
    },
    #[serde(rename = "performance")]
    Performance {
        s: String,
        p: String,
        ts: i64,
        pf: PerformanceMetrics,
    },
}

impl RawPayload {
    pub fn site_id(&self) -> &str {
        match self {
            RawPayload::Pageview { s, .. }
            | RawPayload::Event { s, .. }
            | RawPayload::Performance { s, .. } => s,
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            RawPayload::Pageview { .. } => "pageview",
            RawPayload::Event { .. } => "event",
            RawPayload::Performance { .. } => "performance",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lcp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fcp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cls: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttfb: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub pageviews: i64,
    pub events: i64,
    pub top_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeseriesPoint {
    pub bucket: chrono::DateTime<chrono::Utc>,
    pub pageviews: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopRow {
    pub key: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Vitals {
    #[serde(rename = "lcpP75", skip_serializing_if = "Option::is_none")]
    pub lcp_p75: Option<f64>,
    #[serde(rename = "fcpP75", skip_serializing_if = "Option::is_none")]
    pub fcp_p75: Option<f64>,
    #[serde(rename = "clsP75", skip_serializing_if = "Option::is_none")]
    pub cls_p75: Option<f64>,
    #[serde(rename = "inpP75", skip_serializing_if = "Option::is_none")]
    pub inp_p75: Option<f64>,
    #[serde(rename = "ttfbP75", skip_serializing_if = "Option::is_none")]
    pub ttfb_p75: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub enum TopDimension {
    Path,
    Referrer,
    Country,
    Device,
}

impl TopDimension {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "path" => Some(Self::Path),
            "referrer" => Some(Self::Referrer),
            "country" => Some(Self::Country),
            "device" => Some(Self::Device),
            _ => None,
        }
    }

    pub fn column(&self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Referrer => "referrer",
            Self::Country => "country",
            Self::Device => "device",
        }
    }
}
