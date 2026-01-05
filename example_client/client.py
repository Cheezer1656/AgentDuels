from time import sleep

from agentduels import AgentDuelsClient

while True:
    client = AgentDuelsClient()

    def on_tick(tick):
        if tick == 1:
            client.select_item("Block")
            client.rotate(0.0, -0.6)
            client.place_block()
        elif tick == 2:
            client.select_item("Bow")
        elif tick == 28:
            client.select_item("Sword")

        if 2 <= tick <= 27:
            client.rotate(0.0, -0.2)
            client.use_item()
        elif tick >= 28:
            client.rotate(0.0, 0.0)
            client.move_forward()
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
    except Exception as e:
        print(f"{e.__class__.__name__}: {e}")
        pass
    sleep(1)