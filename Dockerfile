FROM arm32v7/rust:latest
ARG DEBIAN_FRONTEND=noninteractive

RUN apt update && apt upgrade -y
RUN apt-get install -y cmake libopencv-dev clang libclang-dev

WORKDIR /app

CMD ["cargo", "build", "--release"]
