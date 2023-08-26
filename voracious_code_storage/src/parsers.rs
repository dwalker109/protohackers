use std::{path::PathBuf, str::FromStr};

use nom::{
    branch::alt,
    bytes::complete::{tag_no_case, take_till, take_till1},
    character::{
        complete::{digit1, newline, space0, space1},
        is_newline, is_space,
    },
    combinator::{map_res, value, verify},
    sequence::{delimited, preceded, Tuple},
    IResult,
};

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Op {
    Put(PathBuf, usize),
    Get(PathBuf, usize),
    List(PathBuf),
    Help,
    Err(crate::error::Error),
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Op> {
    let (input, op) = alt((put, get, list, help, incomplete))(input)?;

    Ok((input, op))
}

fn put(input: &[u8]) -> IResult<&[u8], Op> {
    let (input, (_, path, len, _)) = (
        tag_no_case("PUT"),
        delimited(
            space1,
            map_res(verify(take_till(is_space), is_valid_filename), |b| {
                PathBuf::from_str(std::str::from_utf8(b).unwrap())
            }),
            space1,
        ),
        map_res(digit1, |b| std::str::from_utf8(b).unwrap().parse::<usize>()),
        newline,
    )
        .parse(input)?;

    Ok((input, Op::Put(path, len)))
}

fn get(input: &[u8]) -> IResult<&[u8], Op> {
    let (input, (_, path, rev)) = (
        tag_no_case("GET"),
        delimited(
            space1,
            map_res(
                verify(
                    take_till1(|b| [b' ', b'\n'].contains(&b)),
                    is_valid_filename,
                ),
                |b| PathBuf::from_str(std::str::from_utf8(b).unwrap()),
            ),
            space0,
        ),
        alt((
            value(usize::MAX, newline),
            preceded(
                tag_no_case("r"),
                map_res(digit1, |b| std::str::from_utf8(b).unwrap().parse::<usize>()),
            ),
        )),
    )
        .parse(input)?;

    Ok((input, Op::Get(path, rev)))
}

fn list(input: &[u8]) -> IResult<&[u8], Op> {
    let (input, (_, path)) = (
        tag_no_case("LIST"),
        delimited(
            space1,
            map_res(verify(take_till(is_newline), is_valid_dirname), |b| {
                PathBuf::from_str(std::str::from_utf8(b).unwrap())
            }),
            newline,
        ),
    )
        .parse(input)?;

    Ok((input, Op::List(path)))
}

fn help(input: &[u8]) -> IResult<&[u8], Op> {
    let (input, _) = tag_no_case("HELP")(input)?;

    Ok((input, Op::Help))
}

fn incomplete(input: &[u8]) -> IResult<&[u8], Op> {
    (alt((
        value(Op::Err(crate::error::Error::Put), tag_no_case("PUT")),
        value(Op::Err(crate::error::Error::Get), tag_no_case("GET")),
        value(Op::Err(crate::error::Error::List), tag_no_case("LIST")),
    )))(input)
}

static NOT_YOU: [char; 24] = [
    '=', '@', '{', ')', '|', '\'', '%', '*', '&', '*', ']', '[', '?', '#', '(', '+', '}', '|', '~',
    '$', '"', '!', '^', '`',
];

fn is_valid_filename(b: &[u8]) -> bool {
    let s = std::str::from_utf8(b).unwrap();
    !s.contains(NOT_YOU) && !s.contains("//") && !s.ends_with('/') && s.starts_with('/')
}

fn is_valid_dirname(b: &[u8]) -> bool {
    let s = std::str::from_utf8(b).unwrap();
    !s.contains(NOT_YOU) && !s.contains("//") && s.starts_with('/')
}
