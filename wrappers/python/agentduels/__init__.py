import json
from socket import AF_INET, socket
from time import sleep


class EventManager:
    def __init__(self):
        self.on_tick = []
        self.on_health_change = []
        self.on_death = []
        self.on_goal = []
        self.on_block_change = []
        self.on_inventory_change = []

    def run_event(event, args):
        for callback in event:
            callback(*args)

    def tick(self, tick):
        EventManager.run_event(self.on_tick, (tick,))

    def health_change(self, player_id, new_health):
        EventManager.run_event(self.on_health_change, (player_id, new_health,))

    def death(self, player_id):
        EventManager.run_event(self.on_death, (player_id,))

    def goal(self, player_id):
        EventManager.run_event(self.on_goal, (player_id,))

    def block_change(self, block_pos, block_type):
        EventManager.run_event(self.on_block_change, (block_pos, block_type))

    def inventory_change(self, player_id):
        EventManager.run_event(self.on_inventory_change, (player_id,))

class Position:
    def __init__(self, x=0, y=0, z=0):
        self.x = x
        self.y = y
        self.z = z

class Rotation:
    def __init__(self, yaw=0.0, pitch=0.0):
        self.yaw = yaw
        self.pitch = pitch

class Item:
    sword = "Sword"
    block = "Block"

class Inventory:
    def __init__(self):
        self.items = {}
        self.selected_item = Item.sword

    def update(self, inventory_data):
        for item_name, count in inventory_data["contents"].items():
            self.items[item_name] = count
        self.selected_item = inventory_data["selected"]

class Actions:
    def __init__(self):
        self.moved_forward = False
        self.moved_backward = False
        self.moved_left = False
        self.moved_right = False
        self.jumped = False
        self.attacked = False
        self.used_item = False
        self.placed_block = False
        self.dug_block = False
        self.rotated = (0.0, 0.0)
        self.item_changed = None

    def update(self, actions_data):
        bits = actions_data["bits"]
        self.moved_forward = bool(bits & (1 << 0))
        self.moved_backward = bool(bits & (1 << 1))
        self.moved_left = bool(bits & (1 << 2))
        self.moved_right = bool(bits & (1 << 3))
        self.jumped = bool(bits & (1 << 4))
        self.attacked = bool(bits & (1 << 5))
        self.used_item = bool(bits & (1 << 6))
        self.placed_block = bool(bits & (1 << 7))
        self.dug_block = bool(bits & (1 << 8))

        rot = actions_data["rotation"]
        self.rotated = Rotation(rot["yaw"], rot["pitch"])

        self.item_changed = actions_data.get("item_change", None)

class Player:
    def __init__(self):
        self.pos = Position()
        self.head_rot = Rotation()
        self.health = float(20)
        self.inventory = Inventory()
        self.actions = None

BLOCKS = {
    "Air": 0,
    "Grass": 1,
    "Dirt": 2,
    "Stone": 3,
    "RedBlock": 4,
    "BlueBlock": 5,
    "WhiteBlock": 6,
}

class Chunk:
    def __init__(self):
        self.blocks = [[[BLOCKS["Air"] for _ in range(16)] for _ in range(16)] for _ in range(16)]

class ChunkMap:
    def __init__(self):
        self.chunks = {}

    def process_pos(x, y, z):
        chunk_x = x // 16
        chunk_y = y // 16
        chunk_z = z // 16
        local_x = x % 16
        local_y = y % 16
        local_z = z % 16
        return (chunk_x, chunk_y, chunk_z), (local_x, local_y, local_z)

    def get_block(self, x, y, z):
        chunk_key, (local_x, local_y, local_z) = ChunkMap.process_pos(x, y, z)
        if chunk_key in self.chunks:
            return self.chunks[chunk_key].blocks[local_x][local_y][local_z]
        return BLOCKS["Air"]

    def set_block(self, x, y, z, block_type):
        chunk_key, (local_x, local_y, local_z) = ChunkMap.process_pos(x, y, z)
        if chunk_key not in self.chunks:
            self.chunks[chunk_key] = Chunk()
        self.chunks[chunk_key].blocks[local_x][local_y][local_z] = block_type

class GameState:
    def __init__(self):
        self.players = {
            0: Player(),
            1: Player()
        }
        self.map = ChunkMap()
        self.scores = {
            0: 0,
            1: 0
        }

class AgentDuelsClient:
    def __init__(self):
        self.state = GameState()
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
            messages = json.loads(response.decode())
            for msg in messages:
                if verbosity > 1 and next(iter(msg)) != "TickStart":
                    print("[*] Received:", msg)
                elif verbosity > 2:
                    print("[*] Received:", msg)
                for key, value in msg.items():
                    if key == "TickStart":
                        self.state.players[1].actions = value["opponent_prev_actions"]
                        self.state.players[1].pos = Position(*value["opponent_position"])
                        self.state.players[0].pos = Position(*value["player_position"])
                        self.events.tick(value["tick"])
                        self.send_message("EndTick", None)
                    elif key == "HealthUpdate":
                        self.state.players[value["player_id"]].health = value["new_health"]
                        self.events.health_change(value["player_id"], value["new_health"])
                    elif key == "Death":
                        self.events.death(value["player_id"])
                    elif key == "Goal":
                        self.state.scores[value["player_id"]] += 1
                        self.events.goal(value["player_id"])
                    elif key == "BlockUpdate":
                        self.state.map.set_block(value[0][0], value[0][1], value[0][2], value[1])
                        self.events.block_change(value[0], value[1])
                    elif key == "InventoryUpdate":
                        self.state.players[value["player_id"]].inventory.update(value["new_contents"])
                        self.events.inventory_change(value["player_id"])
        self.socket.close()