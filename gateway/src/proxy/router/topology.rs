use enumflags2::{bitflags, BitFlags};
use getset::Getters;
use typed_builder::TypedBuilder;

#[bitflags]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TopologyLocationMatch {
    Zone = 1 << 0,
    Node = 1 << 1,
}

impl TopologyLocationMatch {
    pub fn matches(lhs: &TopologyLocation, rhs: &TopologyLocation) -> BitFlags<Self> {
        let mut score = BitFlags::empty();
        if lhs.zone == rhs.zone {
            score |= Self::Zone;
        }
        if lhs.node == rhs.node {
            score |= Self::Node;
        }
        score
    }
}

#[derive(Default, Getters, Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct TopologyLocation {
    #[getset(get = "pub")]
    node: Option<String>,

    #[getset(get = "pub")]
    zone: Option<String>,
}
