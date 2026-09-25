//! Translation of Ruby query keywords into core query settings.
//!
//! Map and set queries select string keys. Deque queries select positions that
//! count from the front. Every option narrows the selection, and `nil` leaves
//! an option unset. `from`, `after`, `to`, and `before` apply in iteration
//! order, so a reverse query starts at the high end. `range` bounds keys or
//! positions in ascending terms and applies in either direction.
//!
//! A bad option raises `ArgumentError` or `TypeError`, as Ruby does for a bad
//! keyword argument.

use super::transient_state_error;
use magnus::r_hash::ForEach;
use magnus::scan_args::scan_args;
use magnus::value::ReprValue;
use magnus::{Error, Integer, RHash, Range, Ruby, StaticSymbol, Symbol, TryConvert, Value};
use prosody::state::{DequeQuery, Direction, ErasedKeyQuery};
use std::num::NonZeroUsize;
use std::ops::Bound;

/// Raw option values from a Ruby keyword hash. `nil` values are absent.
#[derive(Default)]
struct Keywords {
    from: Option<Value>,
    after: Option<Value>,
    to: Option<Value>,
    before: Option<Value>,
    range: Option<Value>,
    prefix: Option<Value>,
    limit: Option<Value>,
}

/// Translated edges and limit. `T` is a key or a position.
///
/// `start` and `end` are in iteration order. `range` is in ascending terms.
struct Bounds<T> {
    start: Bound<T>,
    end: Bound<T>,
    range: Option<(Bound<T>, Bound<T>)>,
    limit: Option<NonZeroUsize>,
}

impl Keywords {
    /// Collects the keyword hash. Deques have no `prefix`.
    fn collect(ruby: &Ruby, options: RHash, prefix_allowed: bool) -> Result<Self, Error> {
        let mut keywords = Self::default();
        options.foreach(|keyword: Symbol, value: Value| {
            let name = keyword.name()?;
            let slot = match name.as_ref() {
                "from" => &mut keywords.from,
                "after" => &mut keywords.after,
                "to" => &mut keywords.to,
                "before" => &mut keywords.before,
                "range" => &mut keywords.range,
                "prefix" if prefix_allowed => &mut keywords.prefix,
                "limit" => &mut keywords.limit,
                other => return Err(argument_error(ruby, format!("unknown keyword: :{other}"))),
            };
            *slot = (!value.is_nil()).then_some(value);
            Ok(ForEach::Continue)
        })?;
        Ok(keywords)
    }

    /// Translates the edges, range, and limit with `convert`.
    fn bounds<T, F>(self, ruby: &Ruby, convert: F) -> Result<Bounds<T>, Error>
    where
        F: Fn(&'static str, Value) -> Result<T, Error>,
    {
        let start = edge(ruby, ("from", self.from), ("after", self.after), &convert)?;
        let end = edge(ruby, ("to", self.to), ("before", self.before), &convert)?;
        let range = match self.range {
            Some(value) => Some(range(ruby, value, &convert)?),
            None => None,
        };
        let limit = match self.limit {
            Some(value) => Some(limit(ruby, value)?),
            None => None,
        };
        Ok(Bounds {
            start,
            end,
            range,
            limit,
        })
    }
}

/// Builds a map or set query from a direction token and optional keywords.
///
/// # Errors
///
/// Raises `ArgumentError` for an unknown keyword, both edges of one side, or a
/// limit that is not a positive `Integer`. Raises
/// `TypeError` for a key that is not a `String` or a range that is not a
/// `Range`. An invalid direction token raises a transient state error.
pub(crate) fn key_query(
    ruby: &Ruby,
    direction: StaticSymbol,
    options: Option<RHash>,
) -> Result<ErasedKeyQuery, Error> {
    let query = ErasedKeyQuery::new().direction(parse_direction(ruby, direction)?);
    let Some(options) = options else {
        return Ok(query);
    };

    let keywords = Keywords::collect(ruby, options, true)?;
    let prefix = keywords.prefix.map(String::try_convert).transpose()?;
    let bounds = keywords.bounds(ruby, |_, value| String::try_convert(value))?;

    let mut query = match bounds.start {
        Bound::Included(key) => query.from(key),
        Bound::Excluded(key) => query.after(key),
        Bound::Unbounded => query,
    };
    query = match bounds.end {
        Bound::Included(key) => query.to(key),
        Bound::Excluded(key) => query.before(key),
        Bound::Unbounded => query,
    };
    if let Some(range) = bounds.range {
        query = query.range(range);
    }
    if let Some(prefix) = prefix {
        query = query.prefix(prefix);
    }
    if let Some(limit) = bounds.limit {
        query = query.limit(limit);
    }
    Ok(query)
}

/// Builds a deque query from a direction token and optional keywords.
///
/// Positions count from the front and must be non-negative. Use a reverse
/// traversal to read from the back.
///
/// # Errors
///
/// Raises `ArgumentError` for an unknown keyword, both edges of one side, a
/// position that is not a non-negative `Integer`, or a limit that is not a
/// positive `Integer`. Raises `TypeError` for a range that
/// is not a `Range`.
pub(crate) fn position_query(
    ruby: &Ruby,
    direction: StaticSymbol,
    options: Option<RHash>,
) -> Result<DequeQuery, Error> {
    let query = DequeQuery::new().direction(parse_direction(ruby, direction)?);
    let Some(options) = options else {
        return Ok(query);
    };

    let bounds = Keywords::collect(ruby, options, false)?
        .bounds(ruby, |keyword, value| position(ruby, keyword, value))?;

    let mut query = match bounds.start {
        Bound::Included(position) => query.from(position),
        Bound::Excluded(position) => query.after(position),
        Bound::Unbounded => query,
    };
    query = match bounds.end {
        Bound::Included(position) => query.to(position),
        Bound::Excluded(position) => query.before(position),
        Bound::Unbounded => query,
    };
    if let Some(range) = bounds.range {
        query = query.range(range);
    }
    if let Some(limit) = bounds.limit {
        query = query.limit(limit);
    }
    Ok(query)
}

/// Splits handle scan arguments: `(direction, options = nil)`.
///
/// # Errors
///
/// Raises `ArgumentError` for a wrong argument count, or `TypeError` for a
/// wrong argument type.
pub(crate) fn scan_arguments(args: &[Value]) -> Result<(StaticSymbol, Option<RHash>), Error> {
    let args = scan_args::<(StaticSymbol,), (Option<RHash>,), (), (), (), ()>(args)?;
    Ok((args.required.0, args.optional.0))
}

/// Splits published reader scan arguments: `(key, direction, options = nil)`.
///
/// # Errors
///
/// Raises `ArgumentError` for a wrong argument count, or `TypeError` for a
/// wrong argument type.
pub(crate) fn published_scan_arguments(
    args: &[Value],
) -> Result<(String, StaticSymbol, Option<RHash>), Error> {
    let args = scan_args::<(String, StaticSymbol), (Option<RHash>,), (), (), (), ()>(args)?;
    let (key, direction) = args.required;
    Ok((key, direction, args.optional.0))
}

/// Parses a traversal direction token into the core [`Direction`].
///
/// An invalid token is a caller mistake and rejects transient.
fn parse_direction(ruby: &Ruby, direction: StaticSymbol) -> Result<Direction, Error> {
    match direction.name()? {
        "forward" => Ok(Direction::Forward),
        "backward" => Ok(Direction::Backward),
        other => Err(transient_state_error(
            ruby,
            format!("direction: expected :forward or :backward, got :{other}"),
        )),
    }
}

/// Translates one side of the selection. The inclusive and exclusive edges of
/// one side are exclusive: an options hash has no call order to pick one.
fn edge<T, F>(
    ruby: &Ruby,
    (inclusive_name, inclusive): (&'static str, Option<Value>),
    (exclusive_name, exclusive): (&'static str, Option<Value>),
    convert: &F,
) -> Result<Bound<T>, Error>
where
    F: Fn(&'static str, Value) -> Result<T, Error>,
{
    match (inclusive, exclusive) {
        (Some(_), Some(_)) => Err(argument_error(
            ruby,
            format!("{inclusive_name}: and {exclusive_name}: are exclusive; pass one of them"),
        )),
        (Some(value), None) => Ok(Bound::Included(convert(inclusive_name, value)?)),
        (None, Some(value)) => Ok(Bound::Excluded(convert(exclusive_name, value)?)),
        (None, None) => Ok(Bound::Unbounded),
    }
}

/// Translates a Ruby `Range`. A `nil` end is unbounded. An exclusive range
/// excludes its end. A descending range is a valid empty range, so it selects
/// nothing.
fn range<T, F>(ruby: &Ruby, value: Value, convert: &F) -> Result<(Bound<T>, Bound<T>), Error>
where
    F: Fn(&'static str, Value) -> Result<T, Error>,
{
    let Some(range) = Range::from_value(value) else {
        return Err(Error::new(
            ruby.exception_type_error(),
            format!("range: expected a Range, got {}", value.inspect()),
        ));
    };

    let low: Value = range.beg()?;
    let low = if low.is_nil() {
        Bound::Unbounded
    } else {
        Bound::Included(convert("range", low)?)
    };
    let high: Value = range.end()?;
    let high = match (high.is_nil(), range.excl()) {
        (true, _) => Bound::Unbounded,
        (false, true) => Bound::Excluded(convert("range", high)?),
        (false, false) => Bound::Included(convert("range", high)?),
    };

    Ok((low, high))
}

/// Translates a front-relative deque position.
fn position(ruby: &Ruby, keyword: &'static str, value: Value) -> Result<usize, Error> {
    let invalid = || {
        argument_error(
            ruby,
            format!(
                "{keyword}: expected a non-negative Integer position, got {}",
                value.inspect()
            ),
        )
    };
    let integer = Integer::from_value(value).ok_or_else(invalid)?;
    integer.to_usize().map_err(|_| invalid())
}

/// Translates a result limit.
fn limit(ruby: &Ruby, value: Value) -> Result<NonZeroUsize, Error> {
    let invalid = || {
        argument_error(
            ruby,
            format!(
                "limit: expected a positive Integer, got {}",
                value.inspect()
            ),
        )
    };
    let integer = Integer::from_value(value).ok_or_else(invalid)?;
    let limit = integer.to_usize().map_err(|_| invalid())?;
    NonZeroUsize::new(limit).ok_or_else(invalid)
}

fn argument_error(ruby: &Ruby, message: String) -> Error {
    Error::new(ruby.exception_arg_error(), message)
}
