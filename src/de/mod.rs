use nom_locate::LocatedSpan;
use nom_tracable::TracableInfo;

pub mod chord;
pub mod score;

type Span<'a> = LocatedSpan<&'a str, TracableInfo>;
type IResult<'a, T> = nom::IResult<Span<'a>, T>;
