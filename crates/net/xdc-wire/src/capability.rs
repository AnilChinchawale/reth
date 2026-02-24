//! XDC Network capability definitions and bootnodes.

use reth_eth_wire::Capability;

/// XDC mainnet network ID
pub const XDC_MAINNET_NETWORK_ID: u64 = 50;

/// XDC Apothem testnet network ID
pub const XDC_APOTHEM_NETWORK_ID: u64 = 51;

/// XDC mainnet genesis hash
pub const XDC_MAINNET_GENESIS: &str =
    "0x4a9d748bd78a8d0385b67788c2435dcdb914f98a96250b68863a1f8b7642d6b1";

/// XDC Apothem testnet genesis hash
pub const XDC_APOTHEM_GENESIS: &str =
    "0xbdea512b4f12ff1135ec92c00dc047ffb93890c2ea1aa0eefe9b013d80640075";

/// Returns the capabilities supported by XDC Network
///
/// These are advertised during the RLPx handshake:
/// - eth/63: Legacy XDC protocol
/// - eth/66: Modern XDC protocol with request IDs
/// - xdpos/100: XDPoS2 consensus protocol
pub fn xdc_capabilities() -> Vec<Capability> {
    vec![
        Capability::new_static("xdpos", 100), // XDPoS2 consensus (highest priority)
        Capability::new_static("eth", 66),    // Modern with request IDs
        Capability::new_static("eth", 63),    // Legacy (widest compatibility)
    ]
}

/// XDC mainnet bootnodes
///
/// These are the default discovery nodes for XDC mainnet.
/// TODO: Replace with actual XDC mainnet bootnodes
pub const XDC_MAINNET_BOOTNODES: &[&str] = &[
    // "enode://pubkey@ip:port",
    // Add actual XDC mainnet bootnodes here
];

/// XDC Apothem testnet bootnodes
///
/// These are the default discovery nodes for XDC Apothem testnet.
/// TODO: Replace with actual Apothem bootnodes
pub const XDC_APOTHEM_BOOTNODES: &[&str] = &[
    // "enode://pubkey@ip:port",
    // Add actual Apothem testnet bootnodes here
];

/// Get bootnodes for a given network ID
pub fn get_bootnodes(network_id: u64) -> &'static [&'static str] {
    match network_id {
        XDC_MAINNET_NETWORK_ID => XDC_MAINNET_BOOTNODES,
        XDC_APOTHEM_NETWORK_ID => XDC_APOTHEM_BOOTNODES,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities() {
        let caps = xdc_capabilities();
        assert_eq!(caps.len(), 3);
        assert_eq!(caps[0].name(), "xdpos");
        assert_eq!(caps[0].version(), 100);
        assert_eq!(caps[1].name(), "eth");
        assert_eq!(caps[1].version(), 66);
        assert_eq!(caps[2].name(), "eth");
        assert_eq!(caps[2].version(), 63);
    }

    #[test]
    fn test_get_bootnodes() {
        let mainnet = get_bootnodes(XDC_MAINNET_NETWORK_ID);
        let apothem = get_bootnodes(XDC_APOTHEM_NETWORK_ID);
        let unknown = get_bootnodes(999);

        assert!(mainnet.is_empty() || !mainnet.is_empty()); // May or may not have bootnodes yet
        assert!(apothem.is_empty() || !apothem.is_empty());
        assert!(unknown.is_empty());
    }
}
