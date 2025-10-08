import json
from socket import AF_INET, socket

ticks = 0
with socket(AF_INET) as s:
    s.connect(("127.0.0.1", 8082))
    print("Connected!")
    while True:
        response = s.recv(1024)
        if response == b"":
            print("Server closed the connection.")
            break
        msg = json.loads(response.decode())
        print("Received:", msg)
        if next(iter(msg)) == "TickStart":
            ticks += 1
            msg = msg["TickStart"]
            if ticks < 20:
                s.send(b'{"MoveForward": null}')
            s.send(b'{"EndTick": null}')