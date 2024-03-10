use nom::error::ErrorKind;
use nom::{IResult, Slice};
use nom_locate::LocatedSpan;
use nom_regex::lib::nom::Err;
use nom_tracable::TracableInfo;
use regex::Regex;

pub type Span<'a> = LocatedSpan<&'a str, TracableInfo>;

pub fn capture(re: Regex) -> impl Fn(Span) -> IResult<Span, Vec<Span>> {
    move |s| {
        if let Some(c) = re.captures(*s) {
            let v: Vec<_> = c
                .iter()
                .flatten()
                .map(|m| s.slice(m.start()..m.end()))
                .collect();
            let offset = {
                let end = v.last().unwrap();
                end.as_ptr() as usize + end.len() - s.as_ptr() as usize
            };
            Ok((s.slice(offset..), v))
        } else {
            Err(Err::Error(nom::error::Error::new(
                s,
                ErrorKind::RegexpCapture,
            )))
        }
    }
}
