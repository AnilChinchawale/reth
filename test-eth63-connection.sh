#!/bin/bash
set -e

echo "=== Testing Reth-XDC eth/63 Protocol Negotiation ==="
echo ""

# Kill old instance
echo "Stopping any running xdc-reth instances..."
pkill -f xdc-reth || true
sleep 3

# Clean data directory
echo "Cleaning data directory..."
rm -rf /mnt/data/mainnet/reth/*

# Start Reth-XDC
echo "Starting xdc-reth with XDC mainnet config..."
nohup /root/.openclaw/workspace/reth-xdc/target/release/xdc-reth node \
  --chain xdc-mainnet \
  --datadir /mnt/data/mainnet/reth \
  --http --http.port 7073 --http.addr 0.0.0.0 \
  --http.api eth,net,web3,admin \
  --port 40303 \
  --discovery.port 40304 \
  > /mnt/data/mainnet/reth/reth.log 2>&1 &

RETH_PID=$!
echo "Started xdc-reth (PID: $RETH_PID)"
echo ""

# Wait for startup
echo "Waiting for RPC to be ready..."
for i in {1..30}; do
  if curl -s http://localhost:7073 -X POST -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"net_version","params":[],"id":1}' > /dev/null 2>&1; then
    echo "RPC is ready!"
    break
  fi
  sleep 1
done

echo ""
echo "=== Initial Status ==="
echo "Network version:"
curl -s http://localhost:7073 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_version","params":[],"id":1}' | jq .

echo ""
echo "Peer count:"
curl -s http://localhost:7073 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}' | jq .

echo ""
echo "=== Adding Local GP5 Peer ==="
echo "Peer enode: enode://6733ac7dcaa84827d1b02eccd9eddfcdf1e25066458c9e1c88ca51853b3c54e10eb3d0a80856ce8a80908332a2f16f98ee1475c534e3aa74b2d75938fd4c329a@127.0.0.1:30307"

ADDPEER_RESULT=$(curl -s http://localhost:7073 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"admin_addPeer","params":["enode://6733ac7dcaa84827d1b02eccd9eddfcdf1e25066458c9e1c88ca51853b3c54e10eb3d0a80856ce8a80908332a2f16f98ee1475c534e3aa74b2d75938fd4c329a@127.0.0.1:30307"],"id":1}')
echo "$ADDPEER_RESULT" | jq .

echo ""
echo "Waiting 15 seconds for peer handshake..."
sleep 15

echo ""
echo "=== After Peer Addition ==="
echo "Peer count:"
curl -s http://localhost:7073 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}' | jq .

echo ""
echo "Block number:"
curl -s http://localhost:7073 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | jq .

echo ""
echo "=== Log Analysis ==="
echo "Checking for handshake and peer messages in logs:"
echo ""
grep -i "session\|handshake\|peer\|eth/63\|capability\|hello" /mnt/data/mainnet/reth/reth.log | tail -30

echo ""
echo "=== Success Criteria ==="
echo "✓ Peers > 0: Check if peer count is greater than 0"
echo "✓ No handshake errors: Check logs above for errors"
echo "✓ Session established: Look for 'session established' in logs"
echo "✓ eth/63 protocol: Look for 'eth/63' capability in logs"

echo ""
echo "=== Test Complete ==="
