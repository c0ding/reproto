// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The `version` module gives you tools to create and compare SemVer-compliant
//! versions.

use crate::errors::Error;
use crate::parser;
#[cfg(feature = "serde")]
use serde::de::{self, Deserialize, Deserializer, Visitor};
#[cfg(feature = "serde")]
use serde::ser::{Serialize, Serializer};
use std::cmp::{self, Ordering};
use std::fmt;
use std::hash;
use std::str;

/// An identifier in the pre-release or build metadata.
///
/// See sections 9 and 10 of the spec for more about pre-release identifers and
/// build metadata.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Identifier {
    /// An identifier that's solely numbers.
    Numeric(u64),
    /// An identifier with letters and numbers.
    AlphaNumeric(String),
}

impl fmt::Display for Identifier {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Identifier::Numeric(ref n) => fmt::Display::fmt(n, f),
            Identifier::AlphaNumeric(ref s) => fmt::Display::fmt(s, f),
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize Identifier as a number or string.
        match *self {
            Identifier::Numeric(n) => serializer.serialize_u64(n),
            Identifier::AlphaNumeric(ref s) => serializer.serialize_str(s),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Identifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdentifierVisitor;

        // Deserialize Identifier from a number or string.
        impl<'de> Visitor<'de> for IdentifierVisitor {
            type Value = Identifier;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a SemVer pre-release or build identifier")
            }

            fn visit_u64<E>(self, numeric: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Identifier::Numeric(numeric))
            }

            fn visit_str<E>(self, alphanumeric: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Identifier::AlphaNumeric(alphanumeric.to_owned()))
            }
        }

        deserializer.deserialize_any(IdentifierVisitor)
    }
}

/// Represents a version number conforming to the semantic versioning scheme.
#[derive(Clone, Eq, Debug)]
pub struct Version {
    /// The major version, to be incremented on incompatible changes.
    pub major: u64,
    /// The minor version, to be incremented when functionality is added in a
    /// backwards-compatible manner.
    pub minor: u64,
    /// The patch version, to be incremented when backwards-compatible bug
    /// fixes are made.
    pub patch: u64,
    /// The pre-release version identifier, if one exists.
    pub pre: Vec<Identifier>,
    /// The build metadata, ignored when determining version precedence.
    pub build: Vec<Identifier>,
}

#[cfg(feature = "serde")]
impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize Version as a string.
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VersionVisitor;

        // Deserialize Version from a string.
        impl<'de> Visitor<'de> for VersionVisitor {
            type Value = Version;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a SemVer version as a string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Version::parse(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(VersionVisitor)
    }
}

impl Version {
    /// Contructs the simple case without pre or build.
    pub fn new(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
            pre: Vec::new(),
            build: Vec::new(),
        }
    }

    /// Parse a string into a semver object.
    ///
    /// # Errors
    ///
    /// Returns an error variant if the input could not be parsed as a semver object.
    ///
    /// In general, this means that the provided string does not conform to the
    /// [semver spec][semver].
    ///
    /// An error for overflow is returned if any numeric component is larger than what can be
    /// stored in `u64`.
    ///
    /// The following are examples for other common error causes:
    ///
    /// * `1.0` - too few numeric components are used. Exactly 3 are expected.
    /// * `1.0.01` - a numeric component has a leading zero.
    /// * `1.0.foo` - uses a non-numeric components where one is expected.
    /// * `1.0.0foo` - metadata is not separated using a legal character like, `+` or `-`.
    /// * `1.0.0+foo_123` - contains metadata with an illegal character (`_`).
    ///   Legal characters for metadata include `a-z`, `A-Z`, `0-9`, `-`, and `.` (dot).
    ///
    /// [semver]: https://semver.org
    pub fn parse(version: &str) -> Result<Version, Error> {
        let mut parser = parser::Parser::new(version)?;
        let version = parser.version()?;

        if !parser.is_eof() {
            return Err(Error::MoreInput);
        }

        Ok(version)
    }

    /// Clears the build metadata
    fn clear_metadata(&mut self) {
        self.build = Vec::new();
        self.pre = Vec::new();
    }

    /// Increments the patch number for this Version (Must be mutable)
    pub fn increment_patch(&mut self) {
        self.patch += 1;
        self.clear_metadata();
    }

    /// Increments the minor version number for this Version (Must be mutable)
    ///
    /// As instructed by section 7 of the spec, the patch number is reset to 0.
    pub fn increment_minor(&mut self) {
        self.minor += 1;
        self.patch = 0;
        self.clear_metadata();
    }

    /// Increments the major version number for this Version (Must be mutable)
    ///
    /// As instructed by section 8 of the spec, the minor and patch numbers are
    /// reset to 0
    pub fn increment_major(&mut self) {
        self.major += 1;
        self.minor = 0;
        self.patch = 0;
        self.clear_metadata();
    }

    /// Checks to see if the current Version is in pre-release status
    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl fmt::Display for Version {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        r#try!(write!(f, "{}.{}.{}", self.major, self.minor, self.patch));
        if !self.pre.is_empty() {
            r#try!(write!(f, "-"));
            for (i, x) in self.pre.iter().enumerate() {
                if i != 0 {
                    r#try!(write!(f, "."))
                }
                r#try!(write!(f, "{}", x));
            }
        }
        if !self.build.is_empty() {
            r#try!(write!(f, "+"));
            for (i, x) in self.build.iter().enumerate() {
                if i != 0 {
                    r#try!(write!(f, "."))
                }
                r#try!(write!(f, "{}", x));
            }
        }
        Ok(())
    }
}

impl cmp::PartialEq for Version {
    #[inline]
    fn eq(&self, other: &Version) -> bool {
        // We should ignore build metadata here, otherwise versions v1 and v2
        // can exist such that !(v1 < v2) && !(v1 > v2) && v1 != v2, which
        // violate strict total ordering rules.
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.pre == other.pre
    }
}

impl cmp::PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            r => return r,
        }

        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            r => return r,
        }

        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            r => return r,
        }

        // NB: semver spec says 0.0.0-pre < 0.0.0
        // but the version of ord defined for vec
        // says that [] < [pre] so we alter it here
        match (self.pre.len(), other.pre.len()) {
            (0, 0) => Ordering::Equal,
            (0, _) => Ordering::Greater,
            (_, 0) => Ordering::Less,
            (_, _) => self.pre.cmp(&other.pre),
        }
    }
}

impl hash::Hash for Version {
    fn hash<H: hash::Hasher>(&self, into: &mut H) {
        self.major.hash(into);
        self.minor.hash(into);
        self.patch.hash(into);
        self.pre.hash(into);
    }
}

impl From<(u64, u64, u64)> for Version {
    fn from(tuple: (u64, u64, u64)) -> Version {
        let (major, minor, patch) = tuple;
        Version::new(major, minor, patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::range::Range;

    #[test]
    fn test_parse() {
        assert_eq!(
            Version::parse("1.2.3"),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Vec::new(),
                build: Vec::new(),
            },)
        );

        assert_eq!(Version::parse("1.2.3"), Ok(Version::new(1, 2, 3)));

        assert_eq!(
            Version::parse("  1.2.3  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Vec::new(),
                build: Vec::new(),
            },)
        );
        assert_eq!(
            Version::parse("1.2.3-alpha1"),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![Identifier::AlphaNumeric(String::from("alpha1"))],
                build: Vec::new(),
            },)
        );
        assert_eq!(
            Version::parse("  1.2.3-alpha1  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![Identifier::AlphaNumeric(String::from("alpha1"))],
                build: Vec::new(),
            },)
        );
        assert_eq!(
            Version::parse("1.2.3+build5"),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Vec::new(),
                build: vec![Identifier::AlphaNumeric(String::from("build5"))],
            },)
        );
        assert_eq!(
            Version::parse("  1.2.3+build5  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Vec::new(),
                build: vec![Identifier::AlphaNumeric(String::from("build5"))],
            },)
        );
        assert_eq!(
            Version::parse("1.2.3-alpha1+build5"),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![Identifier::AlphaNumeric(String::from("alpha1"))],
                build: vec![Identifier::AlphaNumeric(String::from("build5"))],
            },)
        );
        assert_eq!(
            Version::parse("  1.2.3-alpha1+build5  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![Identifier::AlphaNumeric(String::from("alpha1"))],
                build: vec![Identifier::AlphaNumeric(String::from("build5"))],
            },)
        );
        assert_eq!(
            Version::parse("1.2.3-1.alpha1.9+build5.7.3aedf  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![
                    Identifier::Numeric(1),
                    Identifier::AlphaNumeric(String::from("alpha1")),
                    Identifier::Numeric(9),
                ],
                build: vec![
                    Identifier::AlphaNumeric(String::from("build5")),
                    Identifier::Numeric(7),
                    Identifier::AlphaNumeric(String::from("3aedf")),
                ],
            },)
        );
        assert_eq!(
            Version::parse("0.4.0-beta.1+0851523"),
            Ok(Version {
                major: 0,
                minor: 4,
                patch: 0,
                pre: vec![
                    Identifier::AlphaNumeric(String::from("beta")),
                    Identifier::Numeric(1),
                ],
                build: vec![Identifier::AlphaNumeric(String::from("0851523"))],
            },)
        );
    }

    #[test]
    fn test_increment_patch() {
        let mut buggy_release = Version::parse("0.1.0").unwrap();
        buggy_release.increment_patch();
        assert_eq!(buggy_release, Version::parse("0.1.1").unwrap());
    }

    #[test]
    fn test_increment_minor() {
        let mut feature_release = Version::parse("1.4.6").unwrap();
        feature_release.increment_minor();
        assert_eq!(feature_release, Version::parse("1.5.0").unwrap());
    }

    #[test]
    fn test_increment_major() {
        let mut chrome_release = Version::parse("46.1.246773").unwrap();
        chrome_release.increment_major();
        assert_eq!(chrome_release, Version::parse("47.0.0").unwrap());
    }

    #[test]
    fn test_increment_keep_prerelease() {
        let mut release = Version::parse("1.0.0-alpha").unwrap();
        release.increment_patch();

        assert_eq!(release, Version::parse("1.0.1").unwrap());

        release.increment_minor();

        assert_eq!(release, Version::parse("1.1.0").unwrap());

        release.increment_major();

        assert_eq!(release, Version::parse("2.0.0").unwrap());
    }

    #[test]
    fn test_increment_clear_metadata() {
        let mut release = Version::parse("1.0.0+4442").unwrap();
        release.increment_patch();

        assert_eq!(release, Version::parse("1.0.1").unwrap());
        release = Version::parse("1.0.1+hello").unwrap();

        release.increment_minor();

        assert_eq!(release, Version::parse("1.1.0").unwrap());
        release = Version::parse("1.1.3747+hello").unwrap();

        release.increment_major();

        assert_eq!(release, Version::parse("2.0.0").unwrap());
    }

    #[test]
    fn test_eq() {
        assert_eq!(Version::parse("1.2.3"), Version::parse("1.2.3"));
        assert_eq!(
            Version::parse("1.2.3-alpha1"),
            Version::parse("1.2.3-alpha1")
        );
        assert_eq!(
            Version::parse("1.2.3+build.42"),
            Version::parse("1.2.3+build.42")
        );
        assert_eq!(
            Version::parse("1.2.3-alpha1+42"),
            Version::parse("1.2.3-alpha1+42")
        );
        assert_eq!(Version::parse("1.2.3+23"), Version::parse("1.2.3+42"));
    }

    #[test]
    fn test_ne() {
        assert!(Version::parse("0.0.0") != Version::parse("0.0.1"));
        assert!(Version::parse("0.0.0") != Version::parse("0.1.0"));
        assert!(Version::parse("0.0.0") != Version::parse("1.0.0"));
        assert!(Version::parse("1.2.3-alpha") != Version::parse("1.2.3-beta"));
    }

    #[test]
    fn test_show() {
        assert_eq!(
            format!("{}", Version::parse("1.2.3").unwrap()),
            "1.2.3".to_string()
        );
        assert_eq!(
            format!("{}", Version::parse("1.2.3-alpha1").unwrap()),
            "1.2.3-alpha1".to_string()
        );
        assert_eq!(
            format!("{}", Version::parse("1.2.3+build.42").unwrap()),
            "1.2.3+build.42".to_string()
        );
        assert_eq!(
            format!("{}", Version::parse("1.2.3-alpha1+42").unwrap()),
            "1.2.3-alpha1+42".to_string()
        );
    }

    #[test]
    fn test_to_string() {
        assert_eq!(
            Version::parse("1.2.3").unwrap().to_string(),
            "1.2.3".to_string()
        );
        assert_eq!(
            Version::parse("1.2.3-alpha1").unwrap().to_string(),
            "1.2.3-alpha1".to_string()
        );
        assert_eq!(
            Version::parse("1.2.3+build.42").unwrap().to_string(),
            "1.2.3+build.42".to_string()
        );
        assert_eq!(
            Version::parse("1.2.3-alpha1+42").unwrap().to_string(),
            "1.2.3-alpha1+42".to_string()
        );
    }

    #[test]
    fn test_lt() {
        assert!(Version::parse("0.0.0") < Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.0.0") < Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.2.0") < Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.2.3-alpha1") < Version::parse("1.2.3"));
        assert!(Version::parse("1.2.3-alpha1") < Version::parse("1.2.3-alpha2"));
        assert!(!(Version::parse("1.2.3-alpha2") < Version::parse("1.2.3-alpha2")));
        assert!(!(Version::parse("1.2.3+23") < Version::parse("1.2.3+42")));
    }

    #[test]
    fn test_le() {
        assert!(Version::parse("0.0.0") <= Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.0.0") <= Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.2.0") <= Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.2.3-alpha1") <= Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.2.3-alpha2") <= Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.2.3+23") <= Version::parse("1.2.3+42"));
    }

    #[test]
    fn test_gt() {
        assert!(Version::parse("1.2.3-alpha2") > Version::parse("0.0.0"));
        assert!(Version::parse("1.2.3-alpha2") > Version::parse("1.0.0"));
        assert!(Version::parse("1.2.3-alpha2") > Version::parse("1.2.0"));
        assert!(Version::parse("1.2.3-alpha2") > Version::parse("1.2.3-alpha1"));
        assert!(Version::parse("1.2.3") > Version::parse("1.2.3-alpha2"));
        assert!(!(Version::parse("1.2.3-alpha2") > Version::parse("1.2.3-alpha2")));
        assert!(!(Version::parse("1.2.3+23") > Version::parse("1.2.3+42")));
    }

    #[test]
    fn test_ge() {
        assert!(Version::parse("1.2.3-alpha2") >= Version::parse("0.0.0"));
        assert!(Version::parse("1.2.3-alpha2") >= Version::parse("1.0.0"));
        assert!(Version::parse("1.2.3-alpha2") >= Version::parse("1.2.0"));
        assert!(Version::parse("1.2.3-alpha2") >= Version::parse("1.2.3-alpha1"));
        assert!(Version::parse("1.2.3-alpha2") >= Version::parse("1.2.3-alpha2"));
        assert!(Version::parse("1.2.3+23") >= Version::parse("1.2.3+42"));
    }

    #[test]
    fn test_prerelease_check() {
        assert!(Version::parse("1.0.0").unwrap().is_prerelease() == false);
        assert!(Version::parse("0.0.1").unwrap().is_prerelease() == false);
        assert!(Version::parse("4.1.4-alpha").unwrap().is_prerelease());
        assert!(Version::parse("1.0.0-beta294296").unwrap().is_prerelease());
    }

    #[test]
    fn test_spec_order() {
        let vs = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        let mut i = 1;
        while i < vs.len() {
            let a = Version::parse(vs[i - 1]);
            let b = Version::parse(vs[i]);
            assert!(a < b, "nope {:?} < {:?}", a, b);
            i += 1;
        }
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            Version::parse("1.2.3"),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Vec::new(),
                build: Vec::new(),
            },)
        );
        assert_eq!(
            Version::parse("  1.2.3  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Vec::new(),
                build: Vec::new(),
            },)
        );
        assert_eq!(
            Version::parse("1.2.3-alpha1"),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![Identifier::AlphaNumeric(String::from("alpha1"))],
                build: Vec::new(),
            },)
        );
        assert_eq!(
            Version::parse("  1.2.3-alpha1  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![Identifier::AlphaNumeric(String::from("alpha1"))],
                build: Vec::new(),
            },)
        );
        assert_eq!(
            Version::parse("1.2.3+build5"),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Vec::new(),
                build: vec![Identifier::AlphaNumeric(String::from("build5"))],
            },)
        );
        assert_eq!(
            Version::parse("  1.2.3+build5  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: Vec::new(),
                build: vec![Identifier::AlphaNumeric(String::from("build5"))],
            },)
        );
        assert_eq!(
            Version::parse("1.2.3-alpha1+build5"),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![Identifier::AlphaNumeric(String::from("alpha1"))],
                build: vec![Identifier::AlphaNumeric(String::from("build5"))],
            },)
        );
        assert_eq!(
            Version::parse("  1.2.3-alpha1+build5  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![Identifier::AlphaNumeric(String::from("alpha1"))],
                build: vec![Identifier::AlphaNumeric(String::from("build5"))],
            },)
        );
        assert_eq!(
            Version::parse("1.2.3-1.alpha1.9+build5.7.3aedf  "),
            Ok(Version {
                major: 1,
                minor: 2,
                patch: 3,
                pre: vec![
                    Identifier::Numeric(1),
                    Identifier::AlphaNumeric(String::from("alpha1")),
                    Identifier::Numeric(9),
                ],
                build: vec![
                    Identifier::AlphaNumeric(String::from("build5")),
                    Identifier::Numeric(7),
                    Identifier::AlphaNumeric(String::from("3aedf")),
                ],
            },)
        );
        assert_eq!(
            Version::parse("0.4.0-beta.1+0851523"),
            Ok(Version {
                major: 0,
                minor: 4,
                patch: 0,
                pre: vec![
                    Identifier::AlphaNumeric(String::from("beta")),
                    Identifier::Numeric(1),
                ],
                build: vec![Identifier::AlphaNumeric(String::from("0851523"))],
            },)
        );
    }

    struct SemverTest(&'static str, &'static str, bool);

    /// Declare range tests.
    macro_rules! semver_tests {
        ($(($($test:tt)*),)+) => {
            [$(semver_tests!(@test $($test)*),)+]
        };

        (@test $req:expr, $version:expr) => {
            SemverTest($req, $version, false)
        };

        (@test $req:expr, $version:expr, true) => {
            SemverTest($req, $version, true)
        };
    }

    /// Tests ported from:
    /// https://raw.githubusercontent.com/npm/node-semver/master/test/index.js
    #[test]
    fn node_semver_comparisons() {
        let input = semver_tests![
            ("0.0.0", "0.0.0-foo"),
            ("0.0.1", "0.0.0"),
            ("1.0.0", "0.9.9"),
            ("0.10.0", "0.9.0"),
            ("0.99.0", "0.10.0"),
            ("2.0.0", "1.2.3"),
            ("v0.0.0", "0.0.0-foo", true),
            ("v0.0.1", "0.0.0", true),
            ("v1.0.0", "0.9.9", true),
            ("v0.10.0", "0.9.0", true),
            ("v0.99.0", "0.10.0", true),
            ("v2.0.0", "1.2.3", true),
            ("0.0.0", "v0.0.0-foo", true),
            ("0.0.1", "v0.0.0", true),
            ("1.0.0", "v0.9.9", true),
            ("0.10.0", "v0.9.0", true),
            ("0.99.0", "v0.10.0", true),
            ("2.0.0", "v1.2.3", true),
            ("1.2.3", "1.2.3-asdf"),
            ("1.2.3", "1.2.3-4"),
            ("1.2.3", "1.2.3-4-foo"),
            ("1.2.3-5-foo", "1.2.3-5"),
            ("1.2.3-5", "1.2.3-4"),
            ("1.2.3-5-foo", "1.2.3-5-Foo"),
            ("3.0.0", "2.7.2+asdf"),
            ("1.2.3-a.10", "1.2.3-a.5"),
            ("1.2.3-a.b", "1.2.3-a.5"),
            ("1.2.3-a.b", "1.2.3-a"),
            ("1.2.3-a.b.c.10.d.5", "1.2.3-a.b.c.5.d.100"),
            ("1.2.3-r2", "1.2.3-r100"),
            ("1.2.3-r100", "1.2.3-R2"),
        ];

        for (i, &SemverTest(left, right, loose)) in input.iter().enumerate() {
            // NOTE: we don't support loose parsing.
            if loose {
                continue;
            }

            let left = Version::parse(left)
                .map_err(|e| format!("failed to parse: {}: {}", left, e))
                .unwrap();

            let right = Version::parse(right)
                .map_err(|e| format!("failed to parse: {}: {}", right, e))
                .unwrap();

            assert!(left > right, "#{}: {} > {}", i, left, right);
            assert!(!(left < right));
            assert!(!(left <= right));
            assert!(!(left == right));
            assert!(left != right);
        }
    }

    /// Tests ported from:
    /// https://raw.githubusercontent.com/npm/node-semver/master/test/index.js
    #[test]
    fn node_semver_range() {
        // NOTE: we support multiple predicates separated by comma (,) instead of spaces.
        let input = semver_tests![
            // ("1.0.0 - 2.0.0", "1.2.3", ignore),
            ("^1.2.3+build", "1.2.3"),
            ("^1.2.3+build", "1.3.0"),
            // NOTE: not supported.
            // ("1.2.3-pre+asdf - 2.4.3-pre+asdf", "1.2.3"),
            // ("1.2.3pre+asdf - 2.4.3-pre+asdf", "1.2.3", true),
            // ("1.2.3-pre+asdf - 2.4.3pre+asdf", "1.2.3", true),
            // ("1.2.3pre+asdf - 2.4.3pre+asdf", "1.2.3", true),
            // ("1.2.3-pre+asdf - 2.4.3-pre+asdf", "1.2.3-pre.2"),
            // ("1.2.3-pre+asdf - 2.4.3-pre+asdf", "2.4.3-alpha"),
            // ("1.2.3+asdf - 2.4.3+asdf", "1.2.3"),
            ("1.0.0", "1.0.0"),
            (">=*", "0.2.4"),
            ("", "1.0.0"),
            ("*", "1.2.3"),
            ("*", "v1.2.3", true),
            (">=1.0.0", "1.0.0"),
            (">=1.0.0", "1.0.1"),
            (">=1.0.0", "1.1.0"),
            (">1.0.0", "1.0.1"),
            (">1.0.0", "1.1.0"),
            ("<=2.0.0", "2.0.0"),
            ("<=2.0.0", "1.9999.9999"),
            ("<=2.0.0", "0.2.9"),
            ("<2.0.0", "1.9999.9999"),
            ("<2.0.0", "0.2.9"),
            (">= 1.0.0", "1.0.0"),
            (">=  1.0.0", "1.0.1"),
            (">=   1.0.0", "1.1.0"),
            ("> 1.0.0", "1.0.1"),
            (">  1.0.0", "1.1.0"),
            ("<=   2.0.0", "2.0.0"),
            ("<= 2.0.0", "1.9999.9999"),
            ("<=  2.0.0", "0.2.9"),
            ("<    2.0.0", "1.9999.9999"),
            ("<\t2.0.0", "0.2.9"),
            (">=0.1.97", "v0.1.97", true),
            (">=0.1.97", "0.1.97"),
            // ("0.1.20 || 1.2.4", "1.2.4"),
            // (">=0.2.3 || <0.0.1", "0.0.0"),
            // (">=0.2.3 || <0.0.1", "0.2.3"),
            // (">=0.2.3 || <0.0.1", "0.2.4"),
            // ("||", "1.3.4"),
            ("2.x.x", "2.1.3"),
            ("1.2.x", "1.2.3"),
            // ("1.2.x || 2.x", "2.1.3"),
            // ("1.2.x || 2.x", "1.2.3"),
            ("x", "1.2.3"),
            ("2.*.*", "2.1.3"),
            ("1.2.*", "1.2.3"),
            // ("1.2.* || 2.*", "2.1.3"),
            // ("1.2.* || 2.*", "1.2.3"),
            ("*", "1.2.3"),
            ("2", "2.1.2"),
            ("2.3", "2.3.1"),
            ("~x", "0.0.9"),   // >=2.4.0 <2.5.0
            ("~2", "2.0.9"),   // >=2.4.0 <2.5.0
            ("~2.4", "2.4.0"), // >=2.4.0 <2.5.0
            ("~2.4", "2.4.5"),
            // ("~>3.2.1", "3.2.2"), // >=3.2.1 <3.3.0,
            ("~1", "1.2.3"), // >=1.0.0 <2.0.0
            // ("~>1", "1.2.3"),
            // ("~> 1", "1.2.3"),
            ("~1.0", "1.0.2"), // >=1.0.0 <1.1.0,
            ("~ 1.0", "1.0.2"),
            ("~ 1.0.3", "1.0.12"),
            (">=1", "1.0.0"),
            (">= 1", "1.0.0"),
            ("<1.2", "1.1.1"),
            ("< 1.2", "1.1.1"),
            // ("~v0.5.4-pre", "0.5.5"),
            // ("~v0.5.4-pre", "0.5.4"),
            ("=0.7.x", "0.7.2"),
            // NOTE: mixing operations and wildcards are _not_ supported.
            // ("<=0.7.x", "0.7.2"),
            // (">=0.7.x", "0.7.2"),
            // ("<=0.7.x", "0.6.2"),
            ("~1.2.1, >=1.2.3", "1.2.3"),
            ("~1.2.1, =1.2.3", "1.2.3"),
            ("~1.2.1, 1.2.3", "1.2.3"),
            ("~1.2.1, >=1.2.3, 1.2.3", "1.2.3"),
            ("~1.2.1, 1.2.3, >=1.2.3", "1.2.3"),
            ("~1.2.1, 1.2.3", "1.2.3"),
            (">=1.2.1, 1.2.3", "1.2.3"),
            ("1.2.3, >=1.2.1", "1.2.3"),
            (">=1.2.3, >=1.2.1", "1.2.3"),
            (">=1.2.1, >=1.2.3", "1.2.3"),
            (">=1.2", "1.2.8"),
            ("^1.2.3", "1.8.1"),
            ("^0.1.2", "0.1.2"),
            ("^0.1", "0.1.2"),
            ("^0.0.1", "0.0.1"),
            ("^1.2", "1.4.2"),
            ("^1.2, ^1", "1.4.2"),
            ("^1.2.3-alpha", "1.2.3-pre"),
            ("^1.2.0-alpha", "1.2.0-pre"),
            ("^0.0.1-alpha", "0.0.1-beta"),
            ("^0.1.1-alpha", "0.1.1-beta"),
            ("^x", "1.2.3"),
            // ("x - 1.0.0", "0.9.7"),
            // ("x - 1.x", "0.9.7"),
            // ("1.0.0 - x", "1.9.7"),
            // ("1.x - x", "1.9.7"),
            // NOTE: mixing operations and wildcards are _not_ supported.
            // ("<=7.x", "7.9.9"),
        ];

        for (i, &SemverTest(req, version, loose)) in input.iter().enumerate() {
            // NOTE: loose mode not supported.
            if loose {
                continue;
            }

            let req = Range::parse(req)
                .map_err(|e| format!("{}: {}", e, req))
                .unwrap();
            let version = Version::parse(version).unwrap();

            assert!(
                req.matches(&version),
                "#{}: ({}).matches({})",
                i,
                req,
                version
            );
        }
    }
}
