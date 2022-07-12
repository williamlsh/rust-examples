run-server:
    @cargo run --bin mini-redis-server -F otel -r -- --port 6379

image:
    @sudo docker build -t mini-redis .

server:
    @sudo docker run -d --rm --name mini-redis-server --network host mini-redis
