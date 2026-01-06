from time import sleep
import traceback

from agentduels import *

while True:
    client = AgentDuelsClient()

    def on_tick(tick):
        if tick == 1:
            client.select_item(Item.BLOCK)
            client.rotate(0.0, -0.6)
            client.place_block()
        elif tick == 2:
            client.select_item(Item.SWORD)

        if tick >= 2:
            client.rotate(0.0, 0.0)
            client.move_forward()
            pos = client.state.players[0].pos.get()
            pos[0] = round(pos[0] - 1)  # Look one block ahead
            pos[1] = round(pos[1])
            if client.state.map.get_block(*pos) != Block.AIR:
                print("Block detected ahead, jumping")
                client.jump()
            client.attack()

    def on_health_change(player_id, new_health):
        print(f"Player {player_id} health changed to {new_health}")

    def on_death(player_id):
        print(f"Player {player_id} has died.")

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
        client.start(verbosity=1)
    except ConnectionRefusedError:
        pass
    except Exception:
        traceback.print_exc()
    sleep(1)