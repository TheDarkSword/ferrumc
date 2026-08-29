//! The set of Minecraft protocol versions this server can speak.

use crate::net_types::NetTypesError;

/// A protocol version, named after the first Minecraft release that introduced it.
///
/// Several releases can share one protocol version — 1.21.7 and 1.21.8 both speak 772 — so a
/// variant identifies the wire protocol, not a single release. Variants are declared oldest first,
/// which makes the derived ordering chronological: serializers can ask
/// `version >= ProtocolVersion::V1_21_5` rather than matching every arm.
///
/// The lower bound is 1.21 because Mojang's data generator only emits a packet report from that
/// release onward, and the packet ids for older versions are not derivable from the jar. See
/// `docs/versioning/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtocolVersion {
    V1_21,
    V1_21_2,
    V1_21_4,
    V1_21_5,
    V1_21_6,
    V1_21_7,
    V1_21_9,
    V1_21_11,
    V26_1,
    V26_2,
}

impl ProtocolVersion {
    /// Every supported version, oldest first.
    pub const ALL: [Self; 10] = [
        Self::V1_21,
        Self::V1_21_2,
        Self::V1_21_4,
        Self::V1_21_5,
        Self::V1_21_6,
        Self::V1_21_7,
        Self::V1_21_9,
        Self::V1_21_11,
        Self::V26_1,
        Self::V26_2,
    ];

    /// The version the server's own world model and registries are built for.
    pub const CURRENT: Self = Self::V26_2;

    /// The oldest version a client may connect with.
    pub const OLDEST: Self = Self::V1_21;

    /// The number carried in the handshake.
    #[must_use]
    pub const fn number(self) -> i32 {
        match self {
            Self::V1_21 => 767,
            Self::V1_21_2 => 768,
            Self::V1_21_4 => 769,
            Self::V1_21_5 => 770,
            Self::V1_21_6 => 771,
            Self::V1_21_7 => 772,
            Self::V1_21_9 => 773,
            Self::V1_21_11 => 774,
            Self::V26_1 => 775,
            Self::V26_2 => 776,
        }
    }

    /// The release name to report back to a client speaking this version. Where several releases
    /// share a protocol version this is the most recent of them, which is what a client of any of
    /// them expects to see in the server list.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::V1_21 => "1.21.1",
            Self::V1_21_2 => "1.21.3",
            Self::V1_21_4 => "1.21.4",
            Self::V1_21_5 => "1.21.5",
            Self::V1_21_6 => "1.21.6",
            Self::V1_21_7 => "1.21.8",
            Self::V1_21_9 => "1.21.10",
            Self::V1_21_11 => "1.21.11",
            Self::V26_1 => "26.1",
            Self::V26_2 => "26.2",
        }
    }

    /// Resolves a handshake's protocol number, or `None` if this server does not speak it.
    #[must_use]
    pub const fn from_number(number: i32) -> Option<Self> {
        match number {
            767 => Some(Self::V1_21),
            768 => Some(Self::V1_21_2),
            769 => Some(Self::V1_21_4),
            770 => Some(Self::V1_21_5),
            771 => Some(Self::V1_21_6),
            772 => Some(Self::V1_21_7),
            773 => Some(Self::V1_21_9),
            774 => Some(Self::V1_21_11),
            775 => Some(Self::V26_1),
            776 => Some(Self::V26_2),
            _ => None,
        }
    }

    /// Index into the per-version tables the packet id codegen produces.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl TryFrom<i32> for ProtocolVersion {
    type Error = NetTypesError;

    fn try_from(number: i32) -> Result<Self, Self::Error> {
        Self::from_number(number).ok_or(NetTypesError::UnsupportedProtocolVersion(number))
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.number())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_order_is_protocol_order() {
        for pair in ProtocolVersion::ALL.windows(2) {
            let (older, newer) = (pair[0], pair[1]);
            assert!(older < newer, "{older:?} should sort before {newer:?}");
            assert!(
                older.number() < newer.number(),
                "{older:?} has protocol {} but sorts before {newer:?} with {}",
                older.number(),
                newer.number()
            );
        }
    }

    #[test]
    fn numbers_round_trip() {
        for version in ProtocolVersion::ALL {
            assert_eq!(ProtocolVersion::from_number(version.number()), Some(version));
        }
    }

    #[test]
    fn unsupported_numbers_are_rejected() {
        for number in [-1, 0, 47, 766, 777, i32::MAX] {
            assert_eq!(ProtocolVersion::from_number(number), None, "{number}");
        }
    }

    #[test]
    fn indices_match_position_in_all() {
        for (position, version) in ProtocolVersion::ALL.iter().enumerate() {
            assert_eq!(version.index(), position);
        }
    }

    #[test]
    fn bounds_are_the_ends_of_the_range() {
        assert_eq!(ProtocolVersion::OLDEST, ProtocolVersion::ALL[0]);
        assert_eq!(
            ProtocolVersion::CURRENT,
            *ProtocolVersion::ALL.last().expect("ALL is not empty")
        );
    }
}
