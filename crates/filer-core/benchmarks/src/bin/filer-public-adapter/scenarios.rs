//! # Browse scenarios
//!
//! Filer-core delivers listing rows a page at a time. `row.first`, the
//! viewport, and the first page therefore share the page's arrival timestamp:
//! that is the first moment a client can use any row, and the adapter does
//! not invent an earlier per-row time. Examined rows are not visible through
//! public events, so `examined` is reported as unavailable, and with no filter
//! the adapter cannot distinguish accepted from emitted rows either.

use filer_core::DirectoryCursor;
use filer_core_benchmarks::{
    AdapterTrace, CanonicalRow, Continuation, Counts, MetricValue, Milestone, OutputScope, Phase,
    UnavailableReason,
};

use crate::session::{AdapterFailure, CoreSession, Page};

/// Scenario ids this adapter can run; any other id reports `not_supported`.
pub const SUPPORTED: [&str; 4] = [
    "browse.fast.first",
    "browse.fast.scale",
    "browse.metadata.first",
    "browse.next",
];

const LAST_CONTINUATION_PAGE: u64 = 40;

/// Runs the measured actions; untimed setup and teardown stay with the caller.
pub async fn run(
    scenario_id: &str,
    trace: &mut AdapterTrace,
    session: &mut CoreSession,
) -> Result<(), AdapterFailure> {
    let page_size = usize::try_from(trace.request().page_size)
        .map_err(|_| AdapterFailure::new("invalid_request", "page_size does not fit usize"))?;
    let viewport_size = usize::try_from(trace.request().viewport_size)
        .map_err(|_| AdapterFailure::new("invalid_request", "viewport_size does not fit usize"))?;
    match scenario_id {
        "browse.next" => continuation(trace, session, page_size, viewport_size).await,
        "browse.metadata.first" => {
            open_to_completion(trace, session, page_size, viewport_size, false).await
        }
        "browse.fast.first" | "browse.fast.scale" => {
            open_to_completion(trace, session, page_size, viewport_size, true).await
        }
        _ => Err(AdapterFailure::new(
            "unsupported_scenario",
            format!("{scenario_id} has no Filer adapter flow"),
        )),
    }
}

/// `open` commits the first page, then follows cursors until the listing ends.
async fn open_to_completion(
    trace: &mut AdapterTrace,
    session: &mut CoreSession,
    page_size: usize,
    viewport_size: usize,
    membership: bool,
) -> Result<(), AdapterFailure> {
    trace.action_started("open");
    let first = session.scan_page(page_size, None).await?;
    let arrived = trace.now_ns();
    let page_rows = first.rows.len() as u64;
    commit_first_page(trace, arrived, &first, viewport_size);
    let mut rows = first.rows;
    let mut cursor = first.next_cursor;
    while let Some(next) = cursor {
        let page = session.scan_page(page_size, Some(next)).await?;
        rows.extend(page.rows);
        cursor = page.next_cursor;
    }
    let completed = trace.now_ns();
    let counts = counts(rows.len() as u64, page_rows);
    trace.milestone(
        completed,
        "open",
        Milestone {
            phase: Phase::ListingCompleted,
            counts: counts.clone(),
            rows,
            scope: if membership {
                OutputScope::Membership
            } else {
                OutputScope::Metadata
            },
            continuation: Continuation::NotApplicable,
        },
    );
    trace.action_completed("open", counts);
    Ok(())
}

/// `open` commits page 1; each `page-NNNN` action requests the next page.
async fn continuation(
    trace: &mut AdapterTrace,
    session: &mut CoreSession,
    page_size: usize,
    viewport_size: usize,
) -> Result<(), AdapterFailure> {
    trace.action_started("open");
    let first = session.scan_page(page_size, None).await?;
    let arrived = trace.now_ns();
    let first_count = first.rows.len() as u64;
    commit_first_page(trace, arrived, &first, viewport_size);
    trace.action_completed("open", counts(first_count, first_count));
    let mut chain = first.rows;
    let mut cursor = first.next_cursor;
    for page_number in 2..=LAST_CONTINUATION_PAGE {
        let action = format!("page-{page_number:04}");
        let next = cursor.take().ok_or_else(|| {
            AdapterFailure::new(
                "continuation_ended_early",
                format!("the listing ended before {action}"),
            )
        })?;
        trace.action_started(&action);
        let page = session.scan_page(page_size, Some(next)).await?;
        let arrived = trace.now_ns();
        let page_count = page.rows.len() as u64;
        cursor = page.next_cursor;
        trace.milestone(
            arrived,
            &action,
            Milestone {
                phase: Phase::PageCommitted,
                counts: counts(page_count, page_count),
                rows: page.rows.clone(),
                scope: OutputScope::Page,
                continuation: continuation_state(cursor.as_ref()),
            },
        );
        chain.extend(page.rows);
        if page_number == LAST_CONTINUATION_PAGE {
            trace.milestone(
                arrived,
                &action,
                Milestone {
                    phase: Phase::ListingCompleted,
                    counts: counts(page_count, page_count),
                    rows: std::mem::take(&mut chain),
                    scope: OutputScope::Membership,
                    continuation: Continuation::NotApplicable,
                },
            );
        }
        trace.action_completed(&action, counts(page_count, page_count));
    }
    Ok(())
}

fn commit_first_page(trace: &mut AdapterTrace, arrived: u64, page: &Page, viewport_size: usize) {
    let emitted = page.rows.len() as u64;
    let viewport = &page.rows[..viewport_size.min(page.rows.len())];
    let commits: [(Phase, &[CanonicalRow], OutputScope, Continuation); 3] = [
        (
            Phase::RowFirst,
            &page.rows[..1.min(page.rows.len())],
            OutputScope::Viewport,
            Continuation::NotApplicable,
        ),
        (
            Phase::ViewportCommitted,
            viewport,
            OutputScope::Viewport,
            Continuation::NotApplicable,
        ),
        (
            Phase::PageCommitted,
            &page.rows,
            OutputScope::Page,
            continuation_state(page.next_cursor.as_ref()),
        ),
    ];
    for (phase, rows, scope, continuation) in commits {
        trace.milestone(
            arrived,
            "open",
            Milestone {
                phase,
                counts: counts(emitted, rows.len() as u64),
                rows: rows.to_vec(),
                scope,
                continuation,
            },
        );
    }
}

fn continuation_state(cursor: Option<&DirectoryCursor>) -> Continuation {
    match cursor {
        Some(_) => Continuation::More,
        None => Continuation::End,
    }
}

fn counts(emitted: u64, visible: u64) -> Counts {
    let unobservable = MetricValue::Unavailable(UnavailableReason::NotObservable);
    Counts {
        examined: unobservable.clone(),
        accepted: unobservable,
        emitted: MetricValue::Observed(emitted),
        visible: MetricValue::Observed(visible),
    }
}
