//! Data-plane DNS metrics (§6 "Wire / query pipeline").
//!
//! Labels are enum-typed — the proc-macro expansion refuses anything else, which keeps the
//! cardinality budget bounded at compile time. Per the architecture contract, hot-path
//! code performs only relaxed-atomic counter/gauge increments and histogram observations.

use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::{exponential_buckets, Histogram};
use prometheus_client::registry::Registry;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct QueryLabels {
    pub proto: Proto,
    pub qtype: QTypeLabel,
    pub rcode: RcodeLabel,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Proto {
    Udp,
    Tcp,
}
impl prometheus_client::encoding::EncodeLabelValue for Proto {
    fn encode(
        &self,
        encoder: &mut prometheus_client::encoding::LabelValueEncoder<'_>,
    ) -> Result<(), std::fmt::Error> {
        prometheus_client::encoding::EncodeLabelValue::encode(
            &match self {
                Self::Udp => "udp",
                Self::Tcp => "tcp",
            },
            encoder,
        )
    }
}

/// A bounded QType label. We enumerate the common ones; everything else becomes `other` to
/// keep cardinality capped regardless of how exotic the query traffic gets.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum QTypeLabel {
    A,
    Aaaa,
    Ns,
    Cname,
    Soa,
    Ptr,
    Mx,
    Txt,
    Srv,
    Caa,
    Axfr,
    Ixfr,
    Any,
    Other,
}
impl QTypeLabel {
    #[must_use]
    pub fn from_wire(t: nebula_wire::QType) -> Self {
        match t.0 {
            1 => Self::A,
            2 => Self::Ns,
            5 => Self::Cname,
            6 => Self::Soa,
            12 => Self::Ptr,
            15 => Self::Mx,
            16 => Self::Txt,
            28 => Self::Aaaa,
            33 => Self::Srv,
            251 => Self::Ixfr,
            252 => Self::Axfr,
            255 => Self::Any,
            257 => Self::Caa,
            _ => Self::Other,
        }
    }
}
impl prometheus_client::encoding::EncodeLabelValue for QTypeLabel {
    fn encode(
        &self,
        encoder: &mut prometheus_client::encoding::LabelValueEncoder<'_>,
    ) -> Result<(), std::fmt::Error> {
        let s = match self {
            Self::A => "A",
            Self::Aaaa => "AAAA",
            Self::Ns => "NS",
            Self::Cname => "CNAME",
            Self::Soa => "SOA",
            Self::Ptr => "PTR",
            Self::Mx => "MX",
            Self::Txt => "TXT",
            Self::Srv => "SRV",
            Self::Caa => "CAA",
            Self::Axfr => "AXFR",
            Self::Ixfr => "IXFR",
            Self::Any => "ANY",
            Self::Other => "other",
        };
        prometheus_client::encoding::EncodeLabelValue::encode(&s, encoder)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum RcodeLabel {
    NoError,
    FormErr,
    ServFail,
    NxDomain,
    NotImp,
    Refused,
    Other,
}
impl RcodeLabel {
    #[must_use]
    pub fn from_rcode(r: nebula_wire::RCode) -> Self {
        match r {
            nebula_wire::RCode::NoError => Self::NoError,
            nebula_wire::RCode::FormErr => Self::FormErr,
            nebula_wire::RCode::ServFail => Self::ServFail,
            nebula_wire::RCode::NxDomain => Self::NxDomain,
            nebula_wire::RCode::NotImp => Self::NotImp,
            nebula_wire::RCode::Refused => Self::Refused,
            _ => Self::Other,
        }
    }
}
impl prometheus_client::encoding::EncodeLabelValue for RcodeLabel {
    fn encode(
        &self,
        encoder: &mut prometheus_client::encoding::LabelValueEncoder<'_>,
    ) -> Result<(), std::fmt::Error> {
        let s = match self {
            Self::NoError => "NOERROR",
            Self::FormErr => "FORMERR",
            Self::ServFail => "SERVFAIL",
            Self::NxDomain => "NXDOMAIN",
            Self::NotImp => "NOTIMP",
            Self::Refused => "REFUSED",
            Self::Other => "OTHER",
        };
        prometheus_client::encoding::EncodeLabelValue::encode(&s, encoder)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct DropLabels {
    pub reason: DropReason,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum DropReason {
    Malformed,
    RateLimited,
    Policy,
    Overload,
    TsigFail,
    AclDeny,
}
impl prometheus_client::encoding::EncodeLabelValue for DropReason {
    fn encode(
        &self,
        encoder: &mut prometheus_client::encoding::LabelValueEncoder<'_>,
    ) -> Result<(), std::fmt::Error> {
        let s = match self {
            Self::Malformed => "malformed",
            Self::RateLimited => "rate_limited",
            Self::Policy => "policy",
            Self::Overload => "overload",
            Self::TsigFail => "tsig_fail",
            Self::AclDeny => "acl_deny",
        };
        prometheus_client::encoding::EncodeLabelValue::encode(&s, encoder)
    }
}

/// The data-plane metric bundle. Cloned into every listener; all inner metric types are
/// internally reference-counted.
#[derive(Debug, Clone)]
pub struct DnsMetrics {
    pub queries: Family<QueryLabels, Counter>,
    pub latency: Family<QueryLabels, Histogram, HistogramCtor>,
    pub dropped: Family<DropLabels, Counter>,
}

/// Constructor for per-label Histograms — buckets span 1 µs .. ~16 ms.
#[derive(Debug, Clone)]
pub struct HistogramCtor;
impl prometheus_client::metrics::family::MetricConstructor<Histogram> for HistogramCtor {
    fn new_metric(&self) -> Histogram {
        Histogram::new(exponential_buckets(1e-6, 2.0, 16))
    }
}

impl DnsMetrics {
    /// Register all metrics into `registry`. Call once at startup.
    pub fn register(registry: &mut Registry) -> Self {
        let m = Self {
            queries: Family::<QueryLabels, Counter>::default(),
            latency: Family::<QueryLabels, Histogram, HistogramCtor>::new_with_constructor(
                HistogramCtor,
            ),
            dropped: Family::<DropLabels, Counter>::default(),
        };
        registry.register(
            "dns_queries_total",
            "Queries received, labelled by transport, qtype (bounded), and rcode (bounded).",
            m.queries.clone(),
        );
        registry.register(
            "dns_query_duration_seconds",
            "In-process query-to-response latency distribution.",
            m.latency.clone(),
        );
        registry.register(
            "dns_dropped_total",
            "Requests dropped before response.",
            m.dropped.clone(),
        );
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_render_with_labels() {
        let mut reg = Registry::with_prefix("nebula");
        let m = DnsMetrics::register(&mut reg);
        let labels = QueryLabels {
            proto: Proto::Udp,
            qtype: QTypeLabel::A,
            rcode: RcodeLabel::NoError,
        };
        m.queries.get_or_create(&labels).inc();
        m.latency.get_or_create(&labels).observe(0.00005);
        let mut out = String::new();
        prometheus_client::encoding::text::encode(&mut out, &reg).unwrap();
        assert!(out.contains("nebula_dns_queries_total"));
        assert!(out.contains("proto=\"udp\""));
        assert!(out.contains("qtype=\"A\""));
        assert!(out.contains("rcode=\"NOERROR\""));
        assert!(out.contains("nebula_dns_query_duration_seconds"));
    }
}
