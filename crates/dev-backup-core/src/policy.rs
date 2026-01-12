use crate::manifest::ManifestRecord;
use anyhow::{Context, Result};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotDecision {
    Anchor,
    Incremental,
}

#[derive(Debug, Clone)]
pub struct PolicyInput {
    pub now: OffsetDateTime,
    pub max_months_between_anchor: i64,
}

impl Default for PolicyInput {
    fn default() -> Self {
        Self {
            now: OffsetDateTime::now_utc(),
            max_months_between_anchor: 12,
        }
    }
}

pub fn decide_snapshot_type(records: &[ManifestRecord], input: PolicyInput) -> Result<SnapshotDecision> {
    if records.is_empty() {
        return Ok(SnapshotDecision::Anchor);
    }

    let last_anchor = records
        .iter()
        .rev()
        .find(|r| r.record_type == "anchor")
        .context("no anchor found in manifest")?;

    let anchor_ts = OffsetDateTime::parse(&last_anchor.ts, &Rfc3339)
        .context("failed to parse anchor timestamp")?;

    let diff_seconds = (input.now - anchor_ts).whole_seconds();
    let diff_months = diff_seconds / 2_592_000; // approx 30 days

    let mut sum_incr: u64 = 0;
    let mut seen_anchor = false;
    for record in records {
        if record == last_anchor {
            seen_anchor = true;
            continue;
        }
        if seen_anchor {
            sum_incr = sum_incr.saturating_add(record.bytes);
        }
    }

    let anchor_bytes = last_anchor.bytes.max(1);

    if diff_months >= input.max_months_between_anchor {
        return Ok(SnapshotDecision::Anchor);
    }

    if sum_incr >= anchor_bytes {
        return Ok(SnapshotDecision::Anchor);
    }

    Ok(SnapshotDecision::Incremental)
}
