use getset::Getters;

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

    pub fn score(&self, current: &TopologyLocation) -> i32 {
        let mut score = 0;

        match (&self.node, &current.node) {
            (Some(node), Some(current_node)) if node == current_node => score += 1,
            _ => {}
        }

        match (&self.zone, &current.zone) {
            (Some(zone), Some(current_zone)) if zone == current_zone => score += 1,
            _ => {}
        }

        score
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(
        TopologyLocation { node: Some("node1".to_string()), zone: Some("zone1".to_string()) },
        TopologyLocation { node: Some("node1".to_string()), zone: Some("zone1".to_string()) },
        2
    )]
    #[case(
        TopologyLocation { node: Some("node1".to_string()), zone: Some("zone1".to_string()) },
        TopologyLocation { node: Some("node1".to_string()), zone: Some("zone2".to_string()) },
        1
    )]
    #[case(
        TopologyLocation { node: Some("node1".to_string()), zone: Some("zone1".to_string()) },
        TopologyLocation { node: Some("node2".to_string()), zone: Some("zone1".to_string()) },
        1
    )]
    #[case(
        TopologyLocation { node: Some("node1".to_string()), zone: Some("zone1".to_string()) },
        TopologyLocation { node: Some("node2".to_string()), zone: Some("zone2".to_string()) },
        0
    )]
    #[case(
        TopologyLocation { node: None, zone: None },
        TopologyLocation { node: None, zone: None },
        0
    )]
    #[case(
        TopologyLocation { node: None, zone: Some("zone1".to_string()) },
        TopologyLocation { node: None, zone: Some("zone1".to_string()) },
        1
    )]
    #[case(
        TopologyLocation { node: Some("node1".to_string()), zone: None },
        TopologyLocation { node: Some("node1".to_string()), zone: None },
        1
    )]
    #[case(
        TopologyLocation { node: Some("node1".to_string()), zone: None },
        TopologyLocation { node: Some("node2".to_string()), zone: None },
        0
    )]
    fn test_topology_location_score(
        #[case] location1: TopologyLocation,
        #[case] location2: TopologyLocation,
        #[case] expected_score: i32,
    ) {
        assert_eq!(location1.score(&location2), expected_score);
    }
}
