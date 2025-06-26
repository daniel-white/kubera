use enumflags2::{BitFlags, bitflags};
use getset::Getters;

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

#[derive(Default, Getters, Debug, Clone, PartialEq, Eq)]
pub struct TopologyLocation {
    #[getset(get = "pub")]
    node: Option<String>,

    #[getset(get = "pub")]
    zone: Option<String>,
}

impl TopologyLocation {
    pub fn new_builder() -> TopologyLocationBuilder {
        TopologyLocationBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct TopologyLocationBuilder {
    node: Option<String>,
    zone: Option<String>,
}

impl TopologyLocationBuilder {
    pub fn build(self) -> TopologyLocation {
        TopologyLocation {
            node: self.node,
            zone: self.zone,
        }
    }

    pub fn on_node(&mut self, node: &Option<String>) -> &mut Self {
        self.node = node.as_ref().cloned();
        self
    }

    pub fn in_zone(&mut self, zone: &Option<String>) -> &mut Self {
        self.zone = zone.as_ref().cloned();
        self
    }
}
