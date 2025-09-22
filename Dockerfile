# Build stage
FROM rust:alpine3.22 AS build
RUN apk add --no-cache musl-dev
WORKDIR /httpcan
COPY . .
RUN cargo build --release

# User creation stage
FROM alpine:3.22 AS files
RUN adduser --disabled-password --gecos "" --home "/nonexistent" --shell "/sbin/nologin" --no-create-home --uid 10001 httpcan

# Final stage
FROM scratch
COPY --from=files --chmod=444 /etc/passwd /etc/group /etc/
COPY --from=build /httpcan/target/release/httpcan /bin/httpcan
COPY --from=build /httpcan/static /httpcan/static
USER httpcan:httpcan
WORKDIR /httpcan
ENTRYPOINT ["/bin/httpcan"]
EXPOSE 8080
