# Build stage
FROM rust@sha256:fa7c28576553c431224a85c897c38f3a6443bd831be37061ab3560d9e797dc82 AS build
RUN apk add --no-cache musl-dev
WORKDIR /httpcan
COPY . .
RUN cargo build --release

# User creation stage
FROM alpine@sha256:a8560b36e8b8210634f77d9f7f9efd7ffa463e380b75e2e74aff4511df3ef88c AS files
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
