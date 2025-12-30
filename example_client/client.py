import json
from socket import AF_INET, socket
from time import sleep, time

while True:
    ticks = 0
    try:
        with socket(AF_INET) as s:
            s.connect(("127.0.0.1", 8082))
            print("Connected!")
            start = time()
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
                    if ticks == 1:
                        s.send(b'{"SelectItem": "Block"}')
                    if ticks == 1:
                        s.send(f'{{"Rotate": [1.5707963, -0.6]}}'.encode())
                    else:
                        s.send(f'{{"Rotate": [0.0, 0.0]}}'.encode())
                    if ticks == 1:
                        s.send(b'{"PlaceBlock": null}')
                    # else:
                    # s.send(b'{"Jump": null}')
                    s.send(b'{"Attack": null}')
                    if 30 < ticks < 154:
                        s.send(b'{"MoveForward": null}')
                    if ticks == 80:
                        s.send(b'{"SelectItem": "Bow"}')
                    s.send(b'{"EndTick": null}')
                    print("Sent actions")
    except ConnectionRefusedError:
        pass
    except ConnectionResetError:
        pass
    except KeyboardInterrupt:
        print(ticks/(time()-start))
        break
    sleep(1)