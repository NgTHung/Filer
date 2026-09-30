//! # Default Name Order
//!
//! Directory listings sort names the way Explorer-style file managers do:
//! case-insensitively, with digit runs compared as numbers, so `alpha` sorts
//! before `Zeta` and `file2` before `file10`. The same order decides where an
//! ordered continuation resumes, so it must be a strict total order. Names that
//! share a main key fall back to fewer leading zeros and then raw bytes, which
//! makes distinct names always differ. ADR 0002
//! (`docs/adr/0002-default-name-order.md`) records the rules and the
//! measurements behind them.
//!
//! One key function defines the main key, so bulk sorts and single comparisons
//! cannot disagree. The key holds each lowercased character as UTF-8 and each
//! digit run as a marker byte, a fixed-width count of significant digits, and
//! those digits, so comparing keys byte by byte gives the main-key order. Bulk
//! sorts derive every row's key once into a shared buffer instead of deriving
//! two per comparison.
//!
//! ```
//! use filer_core::model::node::NodeKind;
//! use filer_core::pipeline::compare_nodes;
//! use filer_core::{Location, LocationRef, NodeEntry, PipelineConfig};
//! use std::cmp::Ordering;
//!
//! let file = |name: &str| {
//!     let location = Location::local(format!("/docs/{name}"));
//!     NodeEntry::from_location_ref(
//!         LocationRef::from_location(&location),
//!         name,
//!         NodeKind::File { extension: None },
//!     )
//! };
//! let config = PipelineConfig::default();
//! assert_eq!(compare_nodes(&config, &file("file2"), &file("file10")), Ordering::Less);
//! assert_eq!(compare_nodes(&config, &file("File1"), &file("file1")), Ordering::Less);
//! assert_eq!(compare_nodes(&config, &file("file1"), &file("file01")), Ordering::Less);
//! ```

use std::cmp::Ordering;

/// Marks a digit run in a key. No character lowercases to an ASCII digit and
/// multi-byte UTF-8 never contains one, so the marker only begins numbers and
/// sorts them where digit characters sit.
const NUMBER_MARKER: u8 = b'0';

/// Compares two names by the full default name order.
pub(crate) fn compare_names(left: &str, right: &str) -> Ordering {
    if left == right {
        return Ordering::Equal;
    }
    name_key(left)
        .cmp(&name_key(right))
        .then_with(|| break_main_key_tie(left, right))
}

/// Compares two names whose keys were already derived by [`push_name_key`].
pub(crate) fn compare_keyed_names(
    left_key: &[u8],
    left: &str,
    right_key: &[u8],
    right: &str,
) -> Ordering {
    left_key
        .cmp(right_key)
        .then_with(|| break_main_key_tie(left, right))
}

pub(crate) fn name_key(name: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(name.len() + 4);
    push_name_key(name, &mut key);
    key
}

/// Appends the main key for `name` to `key`.
pub(crate) fn push_name_key(name: &str, key: &mut Vec<u8>) {
    let bytes = name.as_bytes();
    let mut offset = 0;
    while let Some(&byte) = bytes.get(offset) {
        if byte.is_ascii_digit() {
            let end = digit_run_end(bytes, offset);
            let digits = strip_leading_zeros(&bytes[offset..end]);
            // A name cannot hold 4 GiB of digits, so saturating never merges
            // two real lengths.
            let width = u32::try_from(digits.len()).unwrap_or(u32::MAX);
            key.push(NUMBER_MARKER);
            key.extend_from_slice(&width.to_be_bytes());
            key.extend_from_slice(digits);
            offset = end;
        } else if byte.is_ascii() {
            key.push(byte.to_ascii_lowercase());
            offset += 1;
        } else {
            // Offsets advance by whole characters or ASCII bytes, so a
            // non-ASCII byte here always starts a character.
            let Some(character) = name[offset..].chars().next() else {
                break;
            };
            let mut buffer = [0; 4];
            for lower in character.to_lowercase() {
                key.extend_from_slice(lower.encode_utf8(&mut buffer).as_bytes());
            }
            offset += character.len_utf8();
        }
    }
}

/// Orders names with equal main keys: fewer leading zeros first, then raw
/// bytes, so uppercase sorts before lowercase and distinct names never tie.
fn break_main_key_tie(left: &str, right: &str) -> Ordering {
    leading_zero_order(left, right).then_with(|| left.cmp(right))
}

/// Compares leading-zero counts of aligned digit runs. Equal main keys give
/// both names the same digit runs by value, so the runs line up.
fn leading_zero_order(left: &str, right: &str) -> Ordering {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (0, 0);
    loop {
        i += left[i..]
            .iter()
            .take_while(|byte| !byte.is_ascii_digit())
            .count();
        j += right[j..]
            .iter()
            .take_while(|byte| !byte.is_ascii_digit())
            .count();
        if i >= left.len() || j >= right.len() {
            return Ordering::Equal;
        }
        let (left_end, right_end) = (digit_run_end(left, i), digit_run_end(right, j));
        let left_zeros = left_end - i - strip_leading_zeros(&left[i..left_end]).len();
        let right_zeros = right_end - j - strip_leading_zeros(&right[j..right_end]).len();
        if left_zeros != right_zeros {
            return left_zeros.cmp(&right_zeros);
        }
        (i, j) = (left_end, right_end);
    }
}

fn digit_run_end(bytes: &[u8], start: usize) -> usize {
    start
        + bytes[start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
}

fn strip_leading_zeros(digits: &[u8]) -> &[u8] {
    let zeros = digits.iter().take_while(|&&digit| digit == b'0').count();
    &digits[zeros..]
}
