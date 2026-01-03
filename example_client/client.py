from time import sleep

from agentduels import AgentDuelsClient

while True:
    client = AgentDuelsClient()

    def on_tick(tick):
        if tick == 1:
            client.select_item("Block")
            client.rotate(0.0, -0.6)
            client.place_block()
        else:
            client.rotate(0.0, 0.0)
            client.move_forward()
            client.jump()
            client.attack()

        if tick == 2:
            client.select_item("Sword")

    client.events.on_tick.append(on_tick)

    try:
        client.start(verbosity=1)
    except ConnectionRefusedError:
        pass
    except Exception as e:
        print(f"{e.__class__.__name__}: {e}")
        pass
    sleep(1)