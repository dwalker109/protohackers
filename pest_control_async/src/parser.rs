use crate::message::{Action, Msg, TargetPopulation, VisitPopulation};

use nom::branch::alt;
use nom::bytes::streaming::tag;
use nom::combinator::{consumed, map, map_res};
use nom::multi::{length_count, length_data};
use nom::number::streaming::{be_u32, be_u8};
use nom::sequence::tuple;
use nom::IResult;

pub fn msg(input: &[u8]) -> IResult<&[u8], Msg> {
    alt((
        msg_hello,
        msg_error,
        msg_ok,
        msg_dial_authority,
        msg_target_populations,
        msg_create_policy,
        msg_delete_policy,
        msg_policy_result,
        msg_site_visit,
    ))(input)
}

fn msg_hello(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, protocol, version, _))) =
        consumed(tuple((tag([0x50]), num, str, num, byte)))(input)?;

    let msg = Msg::Hello {
        protocol: protocol.into(),
        version,
    }
    .validate(length, msg_bytes);

    Ok((input, msg))
}

fn msg_error(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, message, _))) =
        consumed(tuple((tag([0x51]), num, str, byte)))(input)?;

    let msg = Msg::err(message).validate(length, msg_bytes);

    Ok((input, msg))
}

fn msg_ok(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, _))) = consumed(tuple((tag([0x52]), num, byte)))(input)?;

    let msg = Msg::Ok.validate(length, msg_bytes);

    Ok((input, msg))
}

fn msg_dial_authority(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, site, _))) =
        consumed(tuple((tag([0x53]), num, num, byte)))(input)?;

    let msg = Msg::DialAuthority { site }.validate(length, msg_bytes);

    Ok((input, msg))
}

fn msg_target_populations(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, site, populations, _))) = consumed(tuple((
        tag([0x54]),
        num,
        num,
        length_count(
            num,
            map(tuple((str, num, num)), |(species, min, max)| {
                TargetPopulation {
                    species: species.into(),
                    min,
                    max,
                }
            }),
        ),
        byte,
    )))(input)?;

    let msg = Msg::TargetPopulations { site, populations }.validate(length, msg_bytes);

    Ok((input, msg))
}

fn msg_create_policy(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, species, action, _))) = consumed(tuple((
        tag([0x55]),
        num,
        str,
        map(
            alt((tag([0x90]), tag([0xa0]))),
            |action: &[u8]| match *action {
                [0x90] => Action::Cull,
                [0xa0] => Action::Conserve,
                _ => unreachable!(),
            },
        ),
        byte,
    )))(input)?;

    let msg = Msg::CreatePolicy {
        species: species.into(),
        action,
    }
    .validate(length, msg_bytes);

    Ok((input, msg))
}

fn msg_delete_policy(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, policy, _))) =
        consumed(tuple((tag([0x56]), num, num, byte)))(input)?;

    let msg = Msg::DeletePolicy { policy }.validate(length, msg_bytes);

    Ok((input, msg))
}

fn msg_policy_result(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, policy, _))) =
        consumed(tuple((tag([0x57]), num, num, byte)))(input)?;

    let msg = Msg::PolicyResult { policy }.validate(length, msg_bytes);

    Ok((input, msg))
}

fn msg_site_visit(input: &[u8]) -> IResult<&[u8], Msg> {
    let (input, (msg_bytes, (_, length, site, populations, _))) = consumed(tuple((
        tag([0x58]),
        num,
        num,
        length_count(
            num,
            map(tuple((str, num)), |(species, count)| VisitPopulation {
                species: species.into(),
                count,
            }),
        ),
        byte,
    )))(input)?;

    let msg = Msg::SiteVisit { site, populations }.validate(length, msg_bytes);

    Ok((input, msg))
}

fn str(input: &[u8]) -> IResult<&[u8], &str> {
    map_res(length_data(be_u32), std::str::from_utf8)(input)
}

fn num(input: &[u8]) -> IResult<&[u8], u32> {
    be_u32(input)
}

fn byte(input: &[u8]) -> IResult<&[u8], u8> {
    be_u8(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    static EXTRA_BYTES: [u8; 3] = [0xff, 0xff, 0xff];

    fn with_extra(input: &[u8]) -> Vec<u8> {
        input.iter().chain(&EXTRA_BYTES).copied().collect()
    }

    #[test]
    fn msg_hello_ok() {
        static HELLO_MSG: [u8; 25] = [
            0x50, 0x00, 0x00, 0x00, 0x19, 0x00, 0x00, 0x00, 0x0b, 0x70, 0x65, 0x73, 0x74, 0x63,
            0x6f, 0x6e, 0x74, 0x72, 0x6f, 0x6c, 0x00, 0x00, 0x00, 0x01, 0xce,
        ];

        let input = with_extra(&HELLO_MSG);

        let result = msg_hello(&input);

        assert_eq!(
            result,
            Ok((
                EXTRA_BYTES.as_ref(),
                Msg::Hello {
                    protocol: "pestcontrol".into(),
                    version: 1
                }
            ))
        );
    }

    #[test]
    fn msg_error_ok() {
        static ERROR_MSG: [u8; 13] = [
            0x51, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x00, 0x00, 0x03, 0x62, 0x61, 0x64, 0x78,
        ];

        let input = with_extra(&ERROR_MSG);

        let result = msg_error(&input);

        assert_eq!(
            result,
            Ok((
                EXTRA_BYTES.as_ref(),
                Msg::Error {
                    message: "bad".into()
                }
            ))
        );
    }

    #[test]
    fn msg_ok_ok() {
        static OK_MSG: [u8; 6] = [0x52, 0x00, 0x00, 0x00, 0x06, 0xa8];

        let input = with_extra(&OK_MSG);

        let result = msg_ok(&input);

        assert_eq!(result, Ok((EXTRA_BYTES.as_ref(), Msg::Ok)));
    }

    #[test]
    fn msg_dial_authority_ok() {
        static DIAL_AUTHORITY_MSG: [u8; 10] =
            [0x53, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x30, 0x39, 0x3a];

        let input = with_extra(&DIAL_AUTHORITY_MSG);

        let result = msg_dial_authority(&input);

        assert_eq!(
            result,
            Ok((EXTRA_BYTES.as_ref(), Msg::DialAuthority { site: 12345 }))
        );
    }

    #[test]
    fn msg_target_populations_ok() {
        static TARGET_POPULATIONS_MSG: [u8; 44] = [
            0x54, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x30, 0x39, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x03, 0x64, 0x6f, 0x67, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03,
            0x00, 0x00, 0x00, 0x03, 0x72, 0x61, 0x74, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x0a, 0x80,
        ];

        let input = with_extra(&TARGET_POPULATIONS_MSG);

        let result = msg_target_populations(&input);

        assert_eq!(
            result,
            Ok((
                EXTRA_BYTES.as_ref(),
                Msg::TargetPopulations {
                    site: 12345,
                    populations: vec![
                        TargetPopulation {
                            species: "dog".into(),
                            min: 1,
                            max: 3
                        },
                        TargetPopulation {
                            species: "rat".into(),
                            min: 0,
                            max: 10
                        }
                    ]
                }
            ))
        );
    }

    #[test]
    fn msg_create_policy_ok() {
        static CREATE_POLICY_MSG: [u8; 14] = [
            0x55, 0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0x03, 0x64, 0x6f, 0x67, 0xa0, 0xc0,
        ];

        let input = with_extra(&CREATE_POLICY_MSG);

        let result = msg_create_policy(&input);

        assert_eq!(
            result,
            Ok((
                EXTRA_BYTES.as_ref(),
                Msg::CreatePolicy {
                    species: "dog".into(),
                    action: Action::Conserve
                }
            ))
        );
    }

    #[test]
    fn msg_delete_policy_ok() {
        static DELETE_POLICY_MSG: [u8; 10] =
            [0x56, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x7b, 0x25];

        let input = with_extra(&DELETE_POLICY_MSG);

        let result = msg_delete_policy(&input);

        assert_eq!(
            result,
            Ok((EXTRA_BYTES.as_ref(), Msg::DeletePolicy { policy: 123 }))
        );
    }

    #[test]
    fn msg_policy_result_ok() {
        static POLICY_RESULT_MSG: [u8; 10] =
            [0x57, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x7b, 0x24];

        let input = with_extra(&POLICY_RESULT_MSG);

        let result = msg_policy_result(&input);

        assert_eq!(
            result,
            Ok((EXTRA_BYTES.as_ref(), Msg::PolicyResult { policy: 123 }))
        );
    }

    #[test]
    fn msg_site_visit_ok() {
        static SITE_VISIT_MSG: [u8; 36] = [
            0x58, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00, 0x30, 0x39, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x03, 0x64, 0x6f, 0x67, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03,
            0x72, 0x61, 0x74, 0x00, 0x00, 0x00, 0x05, 0x8c,
        ];

        let input = with_extra(&SITE_VISIT_MSG);

        let result = msg_site_visit(&input);

        assert_eq!(
            result,
            Ok((
                EXTRA_BYTES.as_ref(),
                Msg::SiteVisit {
                    site: 12345,
                    populations: vec![
                        VisitPopulation {
                            species: "dog".into(),
                            count: 1,
                        },
                        VisitPopulation {
                            species: "rat".into(),
                            count: 5,
                        }
                    ]
                }
            ))
        );
    }
}
