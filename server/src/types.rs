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
        #[serde(default)]
        u: Option<Utm>,
        #[serde(default)]
        vid: Option<String>,
    },
    #[serde(rename = "event")]
    Event {
        s: String,
        p: String,
        ts: i64,
        n: String,
        #[serde(default)]
        pr: Option<HashMap<String, serde_json::Value>>,
        #[serde(default)]
        vid: Option<String>,
    },
    #[serde(rename = "performance")]
    Performance {
        s: String,
        p: String,
        ts: i64,
        pf: PerformanceMetrics,
        #[serde(default)]
        vid: Option<String>,
    },
    #[serde(rename = "pageleave")]
    Pageleave {
        s: String,
        p: String,
        ts: i64,
        dur: i32,
        #[serde(default)]
        vid: Option<String>,
    },
}

pub const MAX_SITE_ID: usize = 64;
pub const MAX_PATH: usize = 2048;
pub const MAX_REFERRER: usize = 253;
pub const MAX_EVENT_NAME: usize = 64;
pub const MAX_UTM: usize = 128;
pub const MAX_VID: usize = 64;

/// UTM campaign tags parsed from the landing URL query string (pageview only).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Utm {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<String>,
}

impl RawPayload {
    /// Validate user-supplied lengths and float sanity. Body size is already
    /// capped at the router layer; this adds per-field caps so a single 16KB
    /// payload can't stuff one giant value into an indexed column.
    pub fn validate(&mut self) -> Result<(), &'static str> {
        let s = self.site_id();
        if s.is_empty() || s.len() > MAX_SITE_ID {
            return Err("invalid site_id");
        }
        let path = match self {
            RawPayload::Pageview { p, .. }
            | RawPayload::Pageleave { p, .. }
            | RawPayload::Performance { p, .. }
            | RawPayload::Event { p, .. } => p.as_str(),
        };
        if path.is_empty() || path.len() > MAX_PATH {
            return Err("invalid path");
        }
        if let RawPayload::Pageview { r: Some(r), .. } = self
            && r.len() > MAX_REFERRER
        {
            return Err("invalid referrer");
        }
        if let RawPayload::Pageview { u: Some(u), .. } = self {
            for field in [&u.s, &u.m, &u.c] {
                if let Some(v) = field
                    && v.len() > MAX_UTM
                {
                    return Err("invalid utm");
                }
            }
        }
        if let RawPayload::Event { n, .. } = self
            && (n.is_empty() || n.len() > MAX_EVENT_NAME)
        {
            return Err("invalid event name");
        }
        if let Some(vid) = self.vid()
            && vid.len() > MAX_VID
        {
            return Err("invalid vid");
        }
        if let RawPayload::Performance { pf, .. } = self {
            // Postgres percentile_cont chokes on NaN; drop non-finite values.
            for v in [
                &mut pf.lcp,
                &mut pf.fcp,
                &mut pf.cls,
                &mut pf.inp,
                &mut pf.ttfb,
            ] {
                if let Some(x) = *v
                    && !x.is_finite()
                {
                    *v = None;
                }
            }
        }
        Ok(())
    }

    pub fn site_id(&self) -> &str {
        match self {
            RawPayload::Pageview { s, .. }
            | RawPayload::Event { s, .. }
            | RawPayload::Performance { s, .. }
            | RawPayload::Pageleave { s, .. } => s,
        }
    }

    pub fn vid(&self) -> Option<&str> {
        match self {
            RawPayload::Pageview { vid, .. }
            | RawPayload::Event { vid, .. }
            | RawPayload::Performance { vid, .. }
            | RawPayload::Pageleave { vid, .. } => vid.as_deref(),
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            RawPayload::Pageview { .. } => "pageview",
            RawPayload::Event { .. } => "event",
            RawPayload::Performance { .. } => "performance",
            RawPayload::Pageleave { .. } => "pageleave",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pv(s: &str, p: &str) -> RawPayload {
        RawPayload::Pageview {
            s: s.into(),
            p: p.into(),
            ts: 0,
            r: None,
            d: None,
            v: None,
            u: None,
            vid: None,
        }
    }

    #[test]
    fn rejects_empty_site() {
        assert!(pv("", "/").validate().is_err());
    }

    #[test]
    fn rejects_oversize_site() {
        assert!(pv(&"x".repeat(MAX_SITE_ID + 1), "/").validate().is_err());
    }

    #[test]
    fn rejects_empty_path() {
        assert!(pv("s", "").validate().is_err());
    }

    #[test]
    fn rejects_oversize_path() {
        assert!(pv("s", &"a".repeat(MAX_PATH + 1)).validate().is_err());
    }

    #[test]
    fn accepts_normal_pageview() {
        assert!(pv("site-1", "/about").validate().is_ok());
    }

    #[test]
    fn rejects_oversize_referrer() {
        let mut p = pv("s", "/");
        if let RawPayload::Pageview { r, .. } = &mut p {
            *r = Some("a".repeat(MAX_REFERRER + 1));
        }
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_empty_event_name() {
        let mut p = RawPayload::Event {
            s: "s".into(),
            p: "/".into(),
            ts: 0,
            n: "".into(),
            pr: None,
            vid: None,
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn rejects_oversize_event_name() {
        let mut p = RawPayload::Event {
            s: "s".into(),
            p: "/".into(),
            ts: 0,
            n: "n".repeat(MAX_EVENT_NAME + 1),
            pr: None,
            vid: None,
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn drops_non_finite_metrics() {
        let mut p = RawPayload::Performance {
            s: "s".into(),
            p: "/".into(),
            ts: 0,
            pf: PerformanceMetrics {
                lcp: Some(f64::NAN),
                fcp: Some(f64::INFINITY),
                cls: Some(0.1),
                inp: Some(f64::NEG_INFINITY),
                ttfb: None,
            },
            vid: None,
        };
        p.validate().unwrap();
        if let RawPayload::Performance { pf, .. } = p {
            assert!(pf.lcp.is_none());
            assert!(pf.fcp.is_none());
            assert_eq!(pf.cls, Some(0.1));
            assert!(pf.inp.is_none());
        } else {
            panic!()
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
    #[serde(rename = "avgTimeOnPageMs", skip_serializing_if = "Option::is_none")]
    pub avg_time_on_page_ms: Option<f64>,
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
    #[serde(rename = "avgDurMs", skip_serializing_if = "Option::is_none")]
    pub avg_dur_ms: Option<f64>,
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
    UtmSource,
    UtmMedium,
    UtmCampaign,
}

impl TopDimension {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "path" => Some(Self::Path),
            "referrer" => Some(Self::Referrer),
            "country" => Some(Self::Country),
            "device" => Some(Self::Device),
            "utm_source" => Some(Self::UtmSource),
            "utm_medium" => Some(Self::UtmMedium),
            "utm_campaign" => Some(Self::UtmCampaign),
            _ => None,
        }
    }

    pub fn column(&self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Referrer => "referrer",
            Self::Country => "country",
            Self::Device => "device",
            Self::UtmSource => "utm_source",
            Self::UtmMedium => "utm_medium",
            Self::UtmCampaign => "utm_campaign",
        }
    }
}
