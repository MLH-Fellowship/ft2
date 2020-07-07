FROM rust:1.44.0

# Copy local code to the container image.
WORKDIR /usr/src/app
COPY . .

RUN cd worker && cargo install --path .

ENV PORT 8080

# Run the web service on container startup.
CMD ["worker"]
