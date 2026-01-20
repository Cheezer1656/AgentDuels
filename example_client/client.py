import traceback
from sys import argv
from time import sleep

from agentduels import *

while True:
    client = AgentDuelsClient()
    stop_attack = False

    def on_tick(tick):
        global stop_attack

        if tick == 1:
            client.select_item(Item.BLOCK)
        if 1 <= tick < 25:
            client.rotate(3.1415, -1.0)
        if 25 <= tick < 35:
            client.rotate(3.1415, -0.5)
        if tick == 20 or tick == 30:
            client.place_block()

        if tick == 35:
            print("Shooting arrow")
            stop_attack = True
            client.select_item(Item.BOW)

        if tick >= 35 and not stop_attack:
            client.rotate(0.0, 0.0)
            client.move_forward()
            pos = client.state.players[0].pos.get()
            pos[0] = round(pos[0] - 1)  # Look one block ahead
            pos[1] = round(pos[1])
            if client.state.map.get_block(*pos) != Block.AIR:
                print("Block detected ahead, jumping")
                client.jump()
            client.attack()

        if stop_attack:
            client.rotate(0.0, 0.2)
            client.use_item()

    def on_health_change(player_id, old_health, new_health):
        global stop_attack
        print(f"Player {player_id}'s health changed: {old_health} -> {new_health}")
        if player_id == client.state.player_id and new_health < old_health:
            print("Took damage, stopping attack.")
            stop_attack = True
            client.select_item(Item.GOLDEN_APPLE)
        elif player_id == client.state.player_id and new_health > old_health:
            print("Health restored, resuming attack.")
            stop_attack = False
            client.select_item(Item.SWORD)
        if stop_attack and player_id != client.state.player_id and new_health < old_health:
            print("Opponent took damage, resuming attack.")
            stop_attack = False
            client.select_item(Item.SWORD)

    def on_death(player_id):
        global stop_attack
        print(f"Player {player_id} has died.")
        if player_id == client.state.player_id and client.state.players[0].pos.distance_to(client.state.players[1].pos) > 10:
            print("Shooting arrow")
            stop_attack = True
            client.select_item(Item.BOW)

    def on_goal(player_id):
        print(f"Player {player_id} has reached the goal! New scores: {client.state.scores}")

    def on_block_change(block_pos, block_type):
        print(f"Block at {block_pos} changed to {block_type}.")

    def on_inventory_change(player_id):
        print(f"Player {player_id}'s inventory has changed.")

    client.events.on_tick.append(on_tick)
    client.events.on_health_change.append(on_health_change)
    client.events.on_death.append(on_death)
    client.events.on_goal.append(on_goal)
    client.events.on_block_change.append(on_block_change)
    client.events.on_inventory_change.append(on_inventory_change)

    try:
        client.start(port=int(argv[1]), verbosity=3)
    except ConnectionRefusedError:
        pass
    except Exception:
        traceback.print_exc()
    sleep(1)