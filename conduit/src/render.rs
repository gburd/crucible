// Copyright 2026 Oxide Computer Company

//! Render collected Oximeter [`Sample`]s into the Prometheus/OpenMetrics text
//! exposition format.
//!
//! Crucible downstairs currently emit only `Cumulative<i64>` counters
//! (connect/write/read/flush), so we map those to Prometheus counters. Other
//! scalar datum types are rendered as gauges; histograms and missing data are
//! skipped (they have no single scalar value to expose).

use std::fmt::Write as _;

use oximeter::Datum;
use oximeter::Field;
use oximeter::Sample;

/// Convert an Oximeter timeseries name (`target:metric`) into a valid
/// Prometheus metric name. Prometheus permits `[a-zA-Z0-9_:]`; the ':' that
/// Oximeter uses as a separator is conventionally reserved for recording
/// rules, so replace it (and any other invalid byte) with '_'.
fn sanitize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for (i, c) in name.chars().enumerate() {
        // A leading digit is invalid; letters, digits and '_' are valid
        // elsewhere. Everything else (notably the ':' separator) becomes '_'.
        let valid = c == '_'
            || c.is_ascii_alphabetic()
            || (c.is_ascii_digit() && i != 0);
        out.push(if valid { c } else { '_' });
    }
    out
}

/// Escape a label value per the Prometheus exposition format: backslash,
/// double-quote, and newline are the only characters that must be escaped.
fn escape_label_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// Extract a single scalar f64 from a datum, or `None` for datum kinds that
/// have no single scalar value (histograms, byte blobs, strings, missing).
fn datum_value(datum: &Datum) -> Option<f64> {
    match datum {
        Datum::Bool(v) => Some(if *v { 1.0 } else { 0.0 }),
        Datum::I8(v) => Some(f64::from(*v)),
        Datum::U8(v) => Some(f64::from(*v)),
        Datum::I16(v) => Some(f64::from(*v)),
        Datum::U16(v) => Some(f64::from(*v)),
        Datum::I32(v) => Some(f64::from(*v)),
        Datum::U32(v) => Some(f64::from(*v)),
        Datum::I64(v) => Some(*v as f64),
        Datum::U64(v) => Some(*v as f64),
        Datum::F32(v) => Some(f64::from(*v)),
        Datum::F64(v) => Some(*v),
        Datum::CumulativeI64(v) => Some(v.value() as f64),
        Datum::CumulativeU64(v) => Some(v.value() as f64),
        Datum::CumulativeF32(v) => Some(f64::from(v.value())),
        Datum::CumulativeF64(v) => Some(v.value()),
        Datum::String(_)
        | Datum::Bytes(_)
        | Datum::HistogramI8(_)
        | Datum::HistogramU8(_)
        | Datum::HistogramI16(_)
        | Datum::HistogramU16(_)
        | Datum::HistogramI32(_)
        | Datum::HistogramU32(_)
        | Datum::HistogramI64(_)
        | Datum::HistogramU64(_)
        | Datum::HistogramF32(_)
        | Datum::HistogramF64(_)
        | Datum::Missing(_) => None,
    }
}

/// Whether a datum is a cumulative counter (Prometheus `counter` vs `gauge`).
fn is_counter(datum: &Datum) -> bool {
    matches!(
        datum,
        Datum::CumulativeI64(_)
            | Datum::CumulativeU64(_)
            | Datum::CumulativeF32(_)
            | Datum::CumulativeF64(_)
    )
}

fn format_labels(fields: &[Field]) -> String {
    if fields.is_empty() {
        return String::new();
    }
    let mut parts = Vec::with_capacity(fields.len());
    for field in fields {
        parts.push(format!(
            "{}=\"{}\"",
            sanitize_name(&field.name),
            escape_label_value(&field.value.to_string())
        ));
    }
    format!("{{{}}}", parts.join(","))
}

/// Render a batch of samples into a single Prometheus exposition document.
///
/// `# TYPE` is emitted once per metric name. Each sample line carries its
/// measurement timestamp in milliseconds, which Prometheus accepts as an
/// optional trailing field.
pub fn to_prometheus(samples: &[Sample]) -> String {
    let mut body = String::new();
    let mut typed: Vec<String> = Vec::new();

    for sample in samples {
        let Some(value) = datum_value(sample.measurement.datum()) else {
            continue;
        };
        let name = sanitize_name(&sample.timeseries_name.to_string());

        if !typed.contains(&name) {
            let kind = if is_counter(sample.measurement.datum()) {
                "counter"
            } else {
                "gauge"
            };
            let _ = writeln!(body, "# TYPE {name} {kind}");
            typed.push(name.clone());
        }

        let labels = format_labels(&sample.fields());
        let ts = sample.measurement.timestamp().timestamp_millis();
        let _ = writeln!(body, "{name}{labels} {value} {ts}");
    }

    body
}

#[cfg(test)]
mod test {
    use super::*;
    use oximeter::types::Cumulative;
    use oximeter::{Metric, Target};
    use uuid::Uuid;

    #[derive(Target)]
    struct CrucibleDownstairs {
        downstairs_uuid: Uuid,
    }

    #[derive(Metric)]
    struct Read {
        #[datum]
        count: Cumulative<i64>,
    }

    #[test]
    fn sanitize_replaces_colon() {
        assert_eq!(
            sanitize_name("crucible_downstairs:read"),
            "crucible_downstairs_read"
        );
    }

    #[test]
    fn escape_handles_quotes_and_backslashes() {
        assert_eq!(escape_label_value(r#"a"b\c"#), r#"a\"b\\c"#);
    }

    #[test]
    fn renders_counter_with_labels_and_timestamp() {
        let target = CrucibleDownstairs {
            downstairs_uuid: Uuid::nil(),
        };
        let metric = Read {
            count: Cumulative::new(7),
        };
        let sample = Sample::new(&target, &metric).unwrap();

        let out = to_prometheus(&[sample]);

        assert!(out.contains("# TYPE crucible_downstairs_read counter"));
        assert!(out.contains(
            "downstairs_uuid=\"00000000-0000-0000-0000-000000000000\""
        ));
        assert!(out.contains(" 7 "));
    }
}
