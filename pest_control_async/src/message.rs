use itertools::Itertools;

#[derive(PartialEq, Debug, Clone)]
pub enum Msg {
    Hello {
        protocol: String,
        version: u32,
    },
    Error {
        message: String,
    },
    Ok,
    DialAuthority {
        site: u32,
    },
    TargetPopulations {
        site: u32,
        populations: Vec<TargetPopulation>,
    },
    CreatePolicy {
        species: String,
        action: Action,
    },
    DeletePolicy {
        policy: u32,
    },
    PolicyResult {
        policy: u32,
    },
    SiteVisit {
        site: u32,
        populations: Vec<VisitPopulation>,
    },
}

#[derive(PartialEq, Debug, Clone)]
pub struct TargetPopulation {
    pub species: String,
    pub min: u32,
    pub max: u32,
}

#[derive(PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum Action {
    Cull = 0x90,
    Conserve = 0xa0,
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct VisitPopulation {
    pub species: String,
    pub count: u32,
}

impl Msg {
    pub fn validate(self, length: u32, msg_bytes: &[u8]) -> Msg {
        match Self::check_length(length, msg_bytes)
            .and_then(|_| Self::check_checksum(msg_bytes))
            .and_then(|_| self.check_hello())
            .and_then(|_| self.check_site_visit())
        {
            Ok(_) => self,
            Err(msg) => msg,
        }
    }

    fn check_length(length: u32, msg_bytes: &[u8]) -> Result<(), Msg> {
        if length != u32::try_from(msg_bytes.len()).expect("message length < u32::MAX") {
            return Err(Msg::Error {
                message: "unexpected message length".into(),
            });
        }

        Ok(())
    }

    fn check_checksum(msg_bytes: &[u8]) -> Result<(), Msg> {
        if msg_bytes
            .iter()
            .map(|b| usize::from(*b))
            .sum::<usize>()
            .rem_euclid(256)
            != 0
        {
            return Err(Msg::Error {
                message: "checksum did not equal 0".into(),
            });
        }

        Ok(())
    }

    fn check_hello(&self) -> Result<(), Msg> {
        if let Msg::Hello { protocol, version } = self {
            if protocol != "pestcontrol" || *version != 1 {
                return Err(Self::err("bad hello"));
            }
        }

        Ok(())
    }

    fn check_site_visit(&self) -> Result<(), Msg> {
        if let Msg::SiteVisit {
            site: _,
            populations,
        } = self
        {
            if populations
                .iter()
                .dedup()
                .counts_by(|vp| vp.species.clone())
                .iter()
                .any(|(_, n)| n > &1)
            {
                return Err(Self::err("site visit contains conflicting observations"));
            }
        }

        Ok(())
    }

    pub fn err(message: &str) -> Self {
        Msg::Error {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msg_check_valid() {
        assert_eq!(Msg::Ok, Msg::Ok.validate(3, &[0x01, 0x02, 0xfd]));
        assert_eq!(
            Msg::Error {
                message: "unexpected message length".into()
            },
            Msg::Ok.validate(2, &[0x01, 0x02, 0xfd])
        );
        assert_eq!(
            Msg::Error {
                message: "checksum did not equal 0".into()
            },
            Msg::Ok.validate(3, &[0x01, 0x02, 0xff])
        );
    }
}
