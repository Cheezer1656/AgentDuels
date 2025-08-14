from socket import AF_INET, socket

with socket(AF_INET) as s:
    s.connect(("127.0.0.1", 8082))
    print("Connected!")
    s.close()
    # while True:
    #     response = s.recv(4096)
    #     if response == b"":
    #         print("Server closed the connection.")
    #         break
    #     print("Received:", response.decode())