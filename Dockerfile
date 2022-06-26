# Compile
FROM rust:1 AS builder
RUN rustup target add x86_64-unknown-linux-musl
COPY . /opt/sekursranko/
RUN cd /opt/sekursranko \
 && cargo build --release --target x86_64-unknown-linux-musl

# Set up runtime container
FROM alpine:3.16
RUN apk update && apk add dumb-init bash moreutils

# Create user
RUN mkdir /sekursranko/ \
 && addgroup -g 1337 -S sekursranko \
 && adduser -u 1337 -S -G sekursranko sekursranko \
 && chown sekursranko:sekursranko /sekursranko/ \
 && chmod 0700 /sekursranko/

# Create volume
VOLUME [ "/sekursranko" ]

# Copy binary
COPY --from=builder /opt/sekursranko/target/x86_64-unknown-linux-musl/release/sekursranko /usr/local/bin/sekursranko

# Copy entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/entrypoint.sh

# Set up default config
COPY --from=builder /opt/sekursranko/config.example.toml /etc/sekursranko/config.toml
RUN sed -i '/listen_on/s/127.0.0.1/[::]/' /etc/sekursranko/config.toml \
 && sed -i '/backup_dir/c\backup_dir = "/sekursranko/"' /etc/sekursranko/config.toml \
 && chown sekursranko:sekursranko /etc/sekursranko/config.toml

# Switch user
WORKDIR /sekursranko
USER sekursranko

# Note: Use dumb-init in order to fulfil our PID 1 responsibilities,
# see https://github.com/Yelp/dumb-init
ENTRYPOINT [ "/usr/bin/dumb-init", "--" ]
CMD [ "/bin/bash", "/usr/local/bin/entrypoint.sh" ]
