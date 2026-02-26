FROM ubuntu:24.04

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Copy the Reth XDC binary from dist directory
COPY dist/xdc-reth /usr/local/bin/xdc-reth
RUN chmod +x /usr/local/bin/xdc-reth

# Create data directory
RUN mkdir -p /data/xdc-reth

# Expose ports
EXPOSE 7073 30303 30303/udp

ENTRYPOINT ["xdc-reth"]
