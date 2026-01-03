import json
from socket import AF_INET, socket
from time import sleep


class EventManager:
    def __init__(self):
        self.on_tick = []
        self.on_death = []
        self.on_goal = []
        self.on_block_change = []

    def run_event(event, args):
        for callback in event:
            callback(*args)

    def tick(self, tick):
        EventManager.run_event(self.on_tick, (tick,))

    def death(self, player_id):
        EventManager.run_event(self.on_death, (player_id,))

    def goal(self, player_id):
        EventManager.run_event(self.on_goal, (player_id,))

    def block_change(self, block_pos, block_type):
        EventManager.run_event(self.on_block_change, (block_pos, block_type))

class AgentDuelsClient:
    def __init__(self):
        self.events = EventManager()

    def send_message(self, message_type, value):
        msg = json.dumps({message_type: value}).encode()
        self.socket.send(msg)

    def move_forward(self):
        self.send_message("MoveForward", None)

    def move_backward(self):
        self.send_message("MoveBackward", None)

    def move_left(self):
        self.send_message("MoveLeft", None)

    def move_right(self):
        self.send_message("MoveRight", None)

    def jump(self):
        self.send_message("Jump", None)

    def rotate(self, yaw: float, pitch: float):
        """
        Rotate the player's head.

        :param yaw: Yaw angle in radians
        :type yaw: float
        :param pitch: Pitch angle in radians
        :type pitch: float
        """
        self.send_message("Rotate", [yaw, pitch])

    def select_item(self, item_name: str):
        self.send_message("SelectItem", item_name)

    def attack(self):
        self.send_message("Attack", None)

    def use_item(self):
        self.send_message("UseItem", None)

    def place_block(self):
        self.send_message("PlaceBlock", None)

    def dig_block(self):
        self.send_message("DigBlock", None)

    def start(self, verbosity=0):
        self.socket = socket(AF_INET)
        self.socket.connect(("127.0.0.1", 8082))
        if verbosity > 0: print("[*] Connected to the server!")
        while True:
            response = self.socket.recv(1024)
            if response == b"":
                if verbosity > 0: print("[*] Server closed the connection.")
                break
            msg = json.loads(response.decode())
            if verbosity > 1: print("[*] Received:", msg)
            for key, value in msg.items():
                if key == "TickStart":
                    self.events.tick(value["tick"])
                    self.send_message("EndTick", None)
                elif key == "PlayerDeath":
                    self.events.death(value["player_id"])
                elif key == "GoalReached":
                    self.events.goal(value["player_id"])
                elif key == "BlockChange":
                    self.events.block_change(value)
        self.socket.close()