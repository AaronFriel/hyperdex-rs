use super::*;

mod hegel_properties;

fn node(id: NodeId, host: &str, control_port: u16, data_port: u16) -> ClusterNode {
    ClusterNode {
        id,
        host: host.to_owned(),
        control_port,
        data_port,
    }
}

#[test]
fn registering_multiple_daemons_updates_layout_nodes() {
    let catalog = InMemoryCatalog::new(vec![node(3, "10.0.0.3", 1982, 2012)], 2);

    assert!(
        catalog
            .register_daemon(node(1, "10.0.0.1", 2982, 3012))
            .unwrap()
    );
    assert!(
        catalog
            .register_daemon(node(7, "10.0.0.7", 3982, 4012))
            .unwrap()
    );

    assert_eq!(
        catalog.layout().unwrap(),
        ClusterLayout {
            replicas: 2,
            nodes: vec![1, 3, 7],
        }
    );
}

#[test]
fn registering_existing_daemon_replaces_its_advertised_ports_once() {
    let catalog = InMemoryCatalog::new(vec![node(9, "10.0.0.9", 1982, 2012)], 1);

    assert!(
        catalog
            .register_daemon(node(9, "10.0.0.9", 3982, 4012))
            .unwrap()
    );
    assert!(
        !catalog
            .register_daemon(node(9, "10.0.0.9", 3982, 4012))
            .unwrap()
    );

    assert_eq!(
        catalog.layout().unwrap(),
        ClusterLayout {
            replicas: 1,
            nodes: vec![9],
        }
    );
}
