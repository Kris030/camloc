# camloc-aruco

## protocol

```
CAMLOC SERVER

loop
    recieve message
        if organizer ping [RECV 0x0b: u8] // "organizer bonk"
            pong [SEND 0x5a: u8] // "server answer"
        if (client) connection request and camera info [RECV 0xcc: u8, x: f64, y: f64, rotation: f64, fov: f64] // "client connect"
            accept, add to clients
        if (client) value update [RECV value: f64]
            update value
            if all values updated
                calculate new position
                notify subscribers


CAMLOC CLIENT

outer loop
    wait for organizer ping [RECV 0x0b: u8] // "organizer bonk"
    reply with pong [SEND 0xca: u8] // "client answer"
    wait for organizer start [RECV 0x60: u8] // "go"
    send image from camera // [SEND TODO]
    recieve camera info and server ip [RECV x: f64, y: f64, rotation: f64, fov: f64]
    connect to server
        send connection request and camera info [SEND 0xcc: u8, x: f64, y: f64, rotation: f64, fov: f64] // "client connect"
    inner loop
        check for organizer command
            if stop [RECV 0x0d: u8] // "organizer die command"
                break (inner)
        find x value
        send value to server [SEND value: f64]


CAMLOC ORGANIZER

loop
    loop through ips
        send ping [SEND 0x0b: u8] // "organizer bonk"
        if offline
            continue
        if server [RECV 0x5a: u8] // "server answer"
            set server ip
        if client [RECV 0xca: u8] // "client answer"
            add to clients

    get user input
    loop through actions
        if client start
            send start [SEND 0x60: u8] // "go"
            recieve image [RECV TODO]
            show image to user and prompt for camera info
            send camera info [SEND x: f64, y: f64, rotation: f64, fov: f64]
        if client stop
            send stop [SEND 0x0d: u8] // "organizer die command"
```

## docker for cross-compilation

#### prerequisites

[QEMU user static](https://github.com/multiarch/qemu-user-static)

```sh
docker run --rm --privileged multiarch/qemu-user-static --reset -p yes
```

#### building

```sh
docker buildx build -t camloc-build .
```

#### running

```sh
# (in project root)
docker run --rm -it --platform linux/arm/v7 -v $PWD:/app camloc-build
```
