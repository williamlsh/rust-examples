relay:
    RUST_LOG=info cargo run -p relay -- --port 4001 --secret-key-seed 0

listen:
    RUST_LOG=info cargo run -p client -- --secret-key-seed 1 --mode listen --relay-address /dns4/$RELAY_SERVER_IP/tcp/4001/p2p/12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN

dial:
    RUST_LOG=info cargo run -p client -- --secret-key-seed 2 --mode dial --relay-address /dns4/$RELAY_SERVER_IP/tcp/4001/p2p/12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN --remote-peer-id 12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X
