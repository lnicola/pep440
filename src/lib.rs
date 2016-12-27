#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate error_chain;
extern crate regex;

pub mod pep440 {
    mod errors {
        error_chain! {
            errors {
                Parse(v: String) {
                    description("unable to parse version string")
                    display("unable to parse version string: '{}'", v)
                }
            }
        }
    }

    use self::errors::*;
    use std::result;
    use std::fmt::{self, Display, Formatter};
    use regex::{self, Captures, Regex};

    #[derive(Debug, PartialEq)]
    pub enum PreReleaseSegment {
        Alpha(u64),
        Beta(u64),
        ReleaseCandidate(u64),
    }

    #[derive(Debug)]
    pub struct Version {
        pub epoch: Option<u64>,
        pub release: Vec<u64>,
        pub pre_release: Option<PreReleaseSegment>,
        pub post_release: Option<u64>,
        pub dev_release: Option<u64>,
        pub local_label: Option<String>,
    }

    impl Version {
        fn parse_helper(captures: Captures) -> Result<Version> {
            let epoch = if let Some(epoch) = captures.at(1) {
                Some(epoch.parse()
                    .chain_err(|| format!("invalid integer value for epoch: {}", epoch))?)
            } else {
                None
            };

            let mut release = vec![];
            if let Some(release_additional_group) = captures.at(2) {
                for val in release_additional_group.split('.') {
                    release.push(val.parse()
                            .chain_err(|| {
                                format!("invalid integer value for release segment: {}", val)
                            })?);
                }
            }

            let pre_val = if let Some(val) = captures.at(6) {
                val.parse().chain_err(|| {
                    format!("invalid integer value for pre-release segment: {}", val)
                })?
            } else {
                0
            };

            let pre = if captures.at(3).is_some() {
                Some(PreReleaseSegment::Alpha(pre_val))
            } else if captures.at(4).is_some() {
                Some(PreReleaseSegment::Beta(pre_val))
            } else if captures.at(5).is_some() {
                Some(PreReleaseSegment::ReleaseCandidate(pre_val))
            } else {
                None
            };

            let post = if captures.at(7).is_some() {
                Some(if let Some(val) = captures.at(8) {
                    val.parse()
                        .chain_err(|| {
                            format!("invalid integer value for post release segment: {}", val)
                        })?
                } else {
                    0
                })
            } else if let Some(val) = captures.at(9) {
                Some(val.parse()
                    .chain_err(|| {
                        format!("invalid integer value for post release segment: {}", val)
                    })?)
            } else {
                None
            };

            let dev = if captures.at(10).is_some() {
                Some(if let Some(val) = captures.at(11) {
                    val.parse()
                        .chain_err(|| {
                            format!("invalid integer value for development release segment: {}",
                                    val)
                        })?
                } else {
                    0
                })
            } else {
                None
            };

            let local_label = captures.at(12).map(|val| {
                val.chars()
                    .map(|c| match c {
                        '_' | '-' => '.',
                        _ => c,
                    })
                    .collect()
            });

            Ok(Version {
                epoch: epoch,
                release: release,
                pre_release: pre,
                post_release: post,
                dev_release: dev,
                local_label: local_label,
            })
        }

        pub fn parse(s: &str) -> Result<Version> {
            lazy_static! {
                static ref RE: result::Result<Regex, regex::Error> = Regex::new(
                    r"(?ix-u) # case insensitive, ignore whitespace, disable Unicode
                    ^\s* # leading whitespace
                    v? # preceding v character
                    (?:(\d+)!)? # epoch
                    (\d+(?:\.\d+)*) # release segments
                    (?: # pre-release segment
                        [._-]? # pre-release separators
                        (?:(a|alpha)|(b|beta)|(rc|c|pre|preview)) # pre-release spelling
                        [._-]? # separator before signifier
                        (\d+)
                             ? # implicit pre-release number
                    )?
                    (?: # post release segment
                        [._-]? # post release separators
                        (post|rev|r) # post release spelling
                        [._-]? # separator before signifier
                        (\d+)
                             ? # implicit post release number
                    |   -(\d+) # implicit post releases
                    )?
                    (?: # development release segment
                        [._-]? # development release separators
                        (dev)
                        (\d+)
                             ? # implicit development release number
                    )?
                    (?: # local version label
                        \+([a-z0-9._-]+) # local version segments
                    )?
                    \s*$ # trailing whitespace");
            }

            match *RE {
                Ok(ref re) => {
                    if let Some(captures) = re.captures(s) {
                        Self::parse_helper(captures)
                    } else {
                        bail!(ErrorKind::Parse(s.to_string()));
                    }
                }
                _ => bail!("unable to create regex"),
            }
        }
    }

    impl Display for Version {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            if let Some(epoch) = self.epoch {
                write!(f, "{}!", epoch)?;
            }

            let len = self.release.len();
            for val in &self.release[0..len - 1] {
                write!(f, "{}.", val)?;
            }
            write!(f, "{}", self.release[len - 1])?;

            match self.pre_release {
                Some(PreReleaseSegment::Alpha(val)) => write!(f, "a{}", val)?,
                Some(PreReleaseSegment::Beta(val)) => write!(f, "b{}", val)?,
                Some(PreReleaseSegment::ReleaseCandidate(val)) => write!(f, "rc{}", val)?,
                None => {}
            };

            if let Some(val) = self.post_release {
                write!(f, ".post{}", val)?;
            }

            if let Some(val) = self.dev_release {
                write!(f, ".dev{}", val)?;
            }

            if let Some(ref val) = self.local_label {
                write!(f, "+{}", val)?;
            }

            Ok(())
        }
    }

    use std::cmp::Ordering;

    impl Ord for Version {
        fn cmp(&self, other: &Self) -> Ordering {
            use std::iter;

            let r = self.epoch.unwrap_or(0).cmp(&other.epoch.unwrap_or(0));
            if r != Ordering::Equal {
                return r;
            }

            if self.release.len() > other.release.len() {
                for (s1, s2) in self.release
                    .iter()
                    .zip(other.release.iter().chain(iter::repeat(&0))) {
                    let r = s1.cmp(s2);
                    if r != Ordering::Equal {
                        return r;
                    }
                }
            } else {
                for (s1, s2) in self.release
                    .iter()
                    .chain(iter::repeat(&0))
                    .zip(other.release.iter()) {
                    let r = s1.cmp(s2);
                    if r != Ordering::Equal {
                        return r;
                    }
                }
            }

            Ordering::Equal
        }
    }

    impl PartialOrd for Version {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl PartialEq for Version {
        fn eq(&self, other: &Self) -> bool {
            self.cmp(other) == Ordering::Equal
        }
    }

    impl Eq for Version {}

    #[cfg(test)]
    mod tests {
        use pep440::{Version, PreReleaseSegment};

        #[test]
        fn parse() {
            let version = Version::parse("1").unwrap();
            assert_eq!(version.epoch, None);
            assert_eq!(version.release, vec![1]);

            let version = Version::parse("1.2.3").unwrap();
            assert_eq!(version.release, vec![1, 2, 3]);

            let version = Version::parse("12!1").unwrap();
            assert_eq!(version.epoch, Some(12));

            let version = Version::parse("1a2").unwrap();
            assert_eq!(version.pre_release, Some(PreReleaseSegment::Alpha(2)));

            let version = Version::parse("1b3").unwrap();
            assert_eq!(version.pre_release, Some(PreReleaseSegment::Beta(3)));

            let version = Version::parse("1rc4").unwrap();
            assert_eq!(version.pre_release,
                       Some(PreReleaseSegment::ReleaseCandidate(4)));

            let version = Version::parse("1.post2").unwrap();
            assert_eq!(version.post_release, Some(2));

            let version = Version::parse("1.dev3").unwrap();
            assert_eq!(version.dev_release, Some(3));

            let version = Version::parse("10!11.12.13a14.post15.dev16").unwrap();
            assert_eq!(version.epoch, Some(10));
            assert_eq!(version.release, vec![11, 12, 13]);
            assert_eq!(version.pre_release, Some(PreReleaseSegment::Alpha(14)));
            assert_eq!(version.post_release, Some(15));
            assert_eq!(version.dev_release, Some(16));
        }

        #[test]
        fn normalization_case_sensitivity() {
            assert!(Version::parse("1.1.RC1").is_ok());
        }

        #[test]
        fn normalization_integer_normalization() {
            let version = Version::parse("01!02.03a04.post05.dev06").unwrap();

            assert_eq!(version.epoch, Some(1));
            assert_eq!(version.release, vec![2, 3]);
            assert_eq!(version.pre_release, Some(PreReleaseSegment::Alpha(4)));
            assert_eq!(version.post_release, Some(5));
            assert_eq!(version.dev_release, Some(6));
        }

        #[test]
        fn normalization_pre_release_separators() {
            assert_eq!(Version::parse("1.1.a1").unwrap().pre_release,
                       Some(PreReleaseSegment::Alpha(1)));
            assert_eq!(Version::parse("1.1-a1").unwrap().pre_release,
                       Some(PreReleaseSegment::Alpha(1)));
            assert_eq!(Version::parse("1.1_a1").unwrap().pre_release,
                       Some(PreReleaseSegment::Alpha(1)));
            assert_eq!(Version::parse("1.1a.1").unwrap().pre_release,
                       Some(PreReleaseSegment::Alpha(1)));
            assert_eq!(Version::parse("1.1a-1").unwrap().pre_release,
                       Some(PreReleaseSegment::Alpha(1)));
            assert_eq!(Version::parse("1.1a_1").unwrap().pre_release,
                       Some(PreReleaseSegment::Alpha(1)));

            assert!(Version::parse("1.1..a1").is_err());
            assert!(Version::parse("1.1a..1").is_err());
        }

        #[test]
        fn normalization_pre_release_spelling() {
            assert_eq!(Version::parse("1.1alpha1").unwrap().pre_release,
                       Some(PreReleaseSegment::Alpha(1)));
            assert_eq!(Version::parse("1.1beta2").unwrap().pre_release,
                       Some(PreReleaseSegment::Beta(2)));
            assert_eq!(Version::parse("1.1c3").unwrap().pre_release,
                       Some(PreReleaseSegment::ReleaseCandidate(3)));
        }

        #[test]
        fn normalization_implicit_pre_release_number() {
            assert_eq!(Version::parse("1.2a").unwrap().pre_release,
                       Some(PreReleaseSegment::Alpha(0)));
        }

        #[test]
        fn normalization_post_release_separators() {
            assert_eq!(Version::parse("1.2post2").unwrap().post_release, Some(2));
            assert_eq!(Version::parse("1.2.post2").unwrap().post_release, Some(2));
            assert_eq!(Version::parse("1.2-post2").unwrap().post_release, Some(2));
            assert_eq!(Version::parse("1.2_post2").unwrap().post_release, Some(2));
            assert_eq!(Version::parse("1.2.post-2").unwrap().post_release, Some(2));

            assert!(Version::parse("1.2..post2").is_err());
            assert!(Version::parse("1.2post..1").is_err());
        }

        #[test]
        fn normalization_post_release_spelling() {
            assert_eq!(Version::parse("1.0-rev4").unwrap().post_release, Some(4));
            assert_eq!(Version::parse("1.0-r4").unwrap().post_release, Some(4));
        }

        #[test]
        fn normalization_implicit_post_release_number() {
            assert_eq!(Version::parse("1.2.post").unwrap().post_release, Some(0));
        }

        #[test]
        fn normalization_implicit_post_releases() {
            assert_eq!(Version::parse("1.0-1").unwrap().post_release, Some(1));

            assert!(Version::parse("1.0-").is_err());
        }

        #[test]
        fn normalization_development_release_separators() {
            assert_eq!(Version::parse("1.2-dev2").unwrap().dev_release, Some(2));
            assert_eq!(Version::parse("1.2dev2").unwrap().dev_release, Some(2));

            assert!(Version::parse("1.2..dev2").is_err());
            assert!(Version::parse("1.2dev-2").is_err());
        }

        #[test]
        fn normalization_implicit_development_release_number() {
            assert_eq!(Version::parse("1.2.dev").unwrap().dev_release, Some(0));
        }

        #[test]
        fn normalization_local_version_segments() {
            assert_eq!(Version::parse("1.0+ubuntu-1").unwrap().local_label,
                       Some(String::from("ubuntu.1")));
            assert_eq!(Version::parse("1.0+ubuntu_1").unwrap().local_label,
                       Some(String::from("ubuntu.1")));
        }

        #[test]
        fn normalization_preceding_v_character() {
            assert!(Version::parse("v1.2.0").is_ok());
            assert!(Version::parse("V1.2.0").is_ok());
        }

        #[test]
        fn normalization_leading_trailing_whitespace() {
            assert!(Version::parse(" \t\n1.2.0\t\n ").is_ok());
        }

        #[test]
        fn format_release() {
            assert_eq!(format!("{}", Version::parse("1.2.0").unwrap()), "1.2.0");
            assert_eq!(format!("{}", Version::parse("1!1.2.0").unwrap()), "1!1.2.0");
        }

        #[test]
        fn format_epoch() {
            assert_eq!(format!("{}", Version::parse("1!1.2.0").unwrap()), "1!1.2.0");
        }

        #[test]
        fn format_pre() {
            assert_eq!(format!("{}", Version::parse("1.2.0a1").unwrap()), "1.2.0a1");
            assert_eq!(format!("{}", Version::parse("1.2.0b1").unwrap()), "1.2.0b1");
            assert_eq!(format!("{}", Version::parse("1.2.0rc1").unwrap()),
                       "1.2.0rc1");
        }

        #[test]
        fn format_post() {
            assert_eq!(format!("{}", Version::parse("1.2.0.post1").unwrap()),
                       "1.2.0.post1");
        }

        #[test]
        fn format_dev() {
            assert_eq!(format!("{}", Version::parse("1.2.0.dev1").unwrap()),
                       "1.2.0.dev1");
        }

        #[test]
        fn format_local() {
            assert_eq!(format!("{}", Version::parse("1.2.0+foo.1").unwrap()),
                       "1.2.0+foo.1");
        }

        #[test]
        fn format_all() {
            assert_eq!(format!("{}", Version::parse("1!2.3.4a5.post6.dev7+foo.1").unwrap()),
                       "1!2.3.4a5.post6.dev7+foo.1");
        }
    }
}
